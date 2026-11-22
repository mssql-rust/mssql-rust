mod auth;
mod config;
mod connection;

mod tls;
#[cfg(any(
    feature = "rustls",
    feature = "native-tls",
    feature = "vendored-openssl"
))]
mod tls_stream;

pub use auth::*;
pub use config::*;
pub(crate) use connection::*;

use crate::tds::stream::ReceivedToken;
use crate::{
    result::ExecuteResult,
    tds::{
        codec::{self, IteratorJoin},
        stream::{QueryStream, TokenStream},
        Collation,
    },
    BulkLoadRequest, ColumnFlag, ColumnOrderHint, IsolationLevel, MetaDataColumn, ProcParam,
    SortOrder, SqlBulkCopyOption, SqlBulkCopyOptions, SqlReadBytes, ToSql,
};
use codec::{
    BatchRequest, ColumnData, PacketHeader, RpcParam, RpcProcId, RpcStatus, TokenRpcRequest,
    TransactionManagerRequest, TypeInfo, VarLenContext, VarLenType,
};
use enumflags2::BitFlags;
use futures_util::io::{AsyncRead, AsyncWrite};
use futures_util::stream::TryStreamExt;
use std::{borrow::Cow, fmt::Debug};

/// `Client` is the main entry point to the SQL Server, providing query
/// execution capabilities.
///
/// A `Client` is created using the [`Config`], defining the needed
/// connection options and capabilities.
///
/// # Example
///
/// ```no_run
/// # use mssql::{Config, AuthMethod};
/// use tokio_util::compat::TokioAsyncWriteCompatExt;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut config = Config::new();
///
/// config.host("0.0.0.0");
/// config.port(1433);
/// config.authentication(AuthMethod::sql_server("SA", "<Mys3cureP4ssW0rD>"));
///
/// let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
/// tcp.set_nodelay(true)?;
/// // Client is ready to use.
/// let client = mssql::Client::connect(config, tcp.compat_write()).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Client<S: AsyncRead + AsyncWrite + Unpin + Send> {
    pub(crate) connection: Connection<S>,
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> Client<S> {
    /// Uses an instance of [`Config`] to specify the connection
    /// options required to connect to the database using an established
    /// tcp connection
    ///
    /// Note: `tcp_stream` is already a connected stream, so options that
    /// only affect how the connection is established - such as
    /// [`Config::multi_subnet_failover`], which only applies to the
    /// [`SqlBrowser::connect_named`](crate::SqlBrowser::connect_named) named-instance connect path - have no
    /// effect here and must be handled by the caller before constructing
    /// `tcp_stream`.
    pub async fn connect(config: Config, tcp_stream: S) -> crate::Result<Client<S>> {
        Ok(Client {
            connection: Connection::connect(config, tcp_stream).await?,
        })
    }

    /// Reset this connection to its login-time state for reuse by a pool.
    ///
    /// The next SQL batch carries the TDS `RESETCONNECTION` status bit. SQL
    /// Server rolls back an open transaction and clears session settings,
    /// temporary objects, cursors and other per-session state before executing
    /// that batch. The reset acknowledgement is fully consumed before this
    /// method returns.
    pub async fn reset(&mut self) -> crate::Result<()> {
        self.connection.flush_stream().await?;

        // A non-empty batch ensures a request is emitted even though the
        // generic packet sender deliberately emits nothing for empty payloads.
        let request = BatchRequest::new(
            "SELECT 1",
            self.connection.context().transaction_descriptor(),
        );
        let id = self.connection.context_mut().next_packet_id();
        self.connection
            .send_reset(PacketHeader::batch(id), request)
            .await?;
        TokenStream::new(&mut self.connection).flush_done().await?;

        Ok(())
    }

    /// Executes SQL statements in the SQL Server, returning the number rows
    /// affected. Useful for `INSERT`, `UPDATE` and `DELETE` statements. The
    /// `query` can define the parameter placement by annotating them with
    /// `@PN`, where N is the index of the parameter, starting from `1`. If
    /// executing multiple queries at a time, delimit them with `;` and refer to
    /// [`ExecuteResult`] how to get results for the separate queries.
    ///
    /// For mapping of Rust types when writing, see the documentation for
    /// [`ToSql`]. For reading data from the database, see the documentation for
    /// [`FromSql`](crate::FromSql).
    ///
    /// This API is not quite suitable for dynamic query parameters. In these
    /// cases using a [`Query`](crate::Query) object might be easier.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use mssql::Config;
    /// # use tokio_util::compat::TokioAsyncWriteCompatExt;
    /// # use std::env;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let c_str = env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or(
    /// #     "server=tcp:localhost,1433;integratedSecurity=true;TrustServerCertificate=true".to_owned(),
    /// # );
    /// # let config = Config::from_ado_string(&c_str)?;
    /// # let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    /// # tcp.set_nodelay(true)?;
    /// # let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;
    /// let results = client
    ///     .execute(
    ///         "INSERT INTO ##Test (id) VALUES (@P1), (@P2), (@P3)",
    ///         &[&1i32, &2i32, &3i32],
    ///     )
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute<'a>(
        &mut self,
        query: impl Into<Cow<'a, str>>,
        params: &[&dyn ToSql],
    ) -> crate::Result<ExecuteResult> {
        self.connection.flush_stream().await?;
        let rpc_params = Self::rpc_params(query);

        let params = params.iter().map(|s| s.to_sql());
        self.rpc_perform_query(RpcProcId::ExecuteSQL, rpc_params, params)
            .await?;

        ExecuteResult::new(&mut self.connection).await
    }

    /// Executes SQL statements in the SQL Server, returning resulting rows.
    /// Useful for `SELECT` statements. The `query` can define the parameter
    /// placement by annotating them with `@PN`, where N is the index of the
    /// parameter, starting from `1`. If executing multiple queries at a time,
    /// delimit them with `;` and refer to [`QueryStream`] on proper stream
    /// handling.
    ///
    /// For mapping of Rust types when writing, see the documentation for
    /// [`ToSql`]. For reading data from the database, see the documentation for
    /// [`FromSql`](crate::FromSql).
    ///
    /// This API can be cumbersome for dynamic query parameters. In these cases,
    /// if fighting too much with the compiler, using a [`Query`](crate::Query)
    /// object might be easier.
    ///
    /// # Example
    ///
    /// ```
    /// # use mssql::Config;
    /// # use tokio_util::compat::TokioAsyncWriteCompatExt;
    /// # use std::env;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let c_str = env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or(
    /// #     "server=tcp:localhost,1433;integratedSecurity=true;TrustServerCertificate=true".to_owned(),
    /// # );
    /// # let config = Config::from_ado_string(&c_str)?;
    /// # let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    /// # tcp.set_nodelay(true)?;
    /// # let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;
    /// let stream = client
    ///     .query(
    ///         "SELECT @P1, @P2, @P3",
    ///         &[&1i32, &2i32, &3i32],
    ///     )
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn query<'a, 'b>(
        &'a mut self,
        query: impl Into<Cow<'b, str>>,
        params: &'b [&'b dyn ToSql],
    ) -> crate::Result<QueryStream<'a>>
    where
        'a: 'b,
    {
        self.start_query(query, params).await?;

        let ts = TokenStream::new(&mut self.connection);
        let mut result = QueryStream::new(ts.try_unfold());
        result.forward_to_metadata().await?;

        Ok(result)
    }

    /// Start a parameterized query and return a `'static` stream which owns
    /// this client. See [`OwnedQueryStream`](crate::OwnedQueryStream).
    pub async fn into_query<'b>(
        mut self,
        query: impl Into<Cow<'b, str>>,
        params: &'b [&'b dyn ToSql],
    ) -> crate::Result<crate::OwnedQueryStream<Self, S>>
    where
        S: 'static,
    {
        self.start_query(query, params).await?;
        Ok(crate::OwnedQueryStream::from_client(self))
    }

    pub(crate) async fn start_query<'b>(
        &mut self,
        query: impl Into<Cow<'b, str>>,
        params: &'b [&'b dyn ToSql],
    ) -> crate::Result<()> {
        self.connection.flush_stream().await?;
        let rpc_params = Self::rpc_params(query);
        let params = params.iter().map(|param| param.to_sql());
        self.rpc_perform_query(RpcProcId::ExecuteSQL, rpc_params, params)
            .await
    }

    /// Execute multiple queries, delimited with `;` and return multiple result
    /// sets; one for each query.
    ///
    /// # Example
    ///
    /// ```
    /// # use mssql::Config;
    /// # use tokio_util::compat::TokioAsyncWriteCompatExt;
    /// # use std::env;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let c_str = env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or(
    /// #     "server=tcp:localhost,1433;integratedSecurity=true;TrustServerCertificate=true".to_owned(),
    /// # );
    /// # let config = Config::from_ado_string(&c_str)?;
    /// # let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    /// # tcp.set_nodelay(true)?;
    /// # let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;
    /// let row = client.simple_query("SELECT 1 AS col").await?.into_row().await?.unwrap();
    /// assert_eq!(Some(1i32), row.get("col"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Warning
    ///
    /// Do not use this with any user specified input. Please resort to prepared
    /// statements using the [`query`] method.
    ///
    /// [`query`]: #method.query
    pub async fn simple_query<'a, 'b>(
        &'a mut self,
        query: impl Into<Cow<'b, str>>,
    ) -> crate::Result<QueryStream<'a>>
    where
        'a: 'b,
    {
        self.start_simple_query(query).await?;

        let ts = TokenStream::new(&mut self.connection);

        let mut result = QueryStream::new(ts.try_unfold());
        result.forward_to_metadata().await?;

        Ok(result)
    }

    /// Start a SQL batch and return a `'static` stream which owns this client.
    pub async fn into_simple_query<'b>(
        mut self,
        query: impl Into<Cow<'b, str>>,
    ) -> crate::Result<crate::OwnedQueryStream<Self, S>>
    where
        S: 'static,
    {
        self.start_simple_query(query).await?;
        Ok(crate::OwnedQueryStream::from_client(self))
    }

    pub(crate) async fn start_simple_query<'b>(
        &mut self,
        query: impl Into<Cow<'b, str>>,
    ) -> crate::Result<()> {
        self.connection.flush_stream().await?;
        let request = BatchRequest::new(query, self.connection.context().transaction_descriptor());
        let id = self.connection.context_mut().next_packet_id();
        self.connection.send(PacketHeader::batch(id), request).await
    }

    /// Call a stored procedure by name, with `OUTPUT` parameters and the
    /// procedure's own `RETURN` value available afterward via
    /// [`QueryStream::into_output_params`]. Unlike [`execute`](Self::execute)/
    /// [`query`](Self::query), which always go through `sp_executesql`, this
    /// calls the procedure directly - the natural way to get real `OUTPUT`
    /// semantics, since `sp_executesql`'s own parameters can't be bound
    /// `OUTPUT` themselves.
    ///
    /// Any result sets the procedure produces (e.g. a `SELECT` before its
    /// `RETURN`) still come through the returned [`QueryStream`] exactly as
    /// with `query` - read those first if you need both.
    ///
    /// An `OUTPUT` parameter's placeholder value (see
    /// [`ProcParam::output`]) establishes its type. It can be a concrete
    /// value (`&0i32`), or a NULL of a type that is sent typed: `bool`, the
    /// integer and float types, and `Uuid` (`&None::<i32>`). Other NULLs
    /// (e.g. `&None::<&str>`) are sent untyped, which SQL Server rejects as
    /// `OUTPUT`. Unlike `execute`/`query`, there is no separate
    /// type-declaration string here for the server to learn the type from.
    /// This is checked client-side, with a clear error, before anything is
    /// sent.
    ///
    /// # Example
    ///
    /// ```
    /// # use mssql::{Config, ProcParam};
    /// # use tokio_util::compat::TokioAsyncWriteCompatExt;
    /// # use std::env;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let c_str = env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or(
    /// #     "server=tcp:localhost,1433;integratedSecurity=true;TrustServerCertificate=true".to_owned(),
    /// # );
    /// # let config = Config::from_ado_string(&c_str)?;
    /// # let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    /// # tcp.set_nodelay(true)?;
    /// # let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;
    /// client.simple_query(
    ///     "CREATE OR ALTER PROCEDURE ##double_it (@n INT, @doubled INT OUTPUT) AS
    ///      BEGIN SET @doubled = @n * 2; RETURN 1; END"
    /// ).await?;
    ///
    /// let stream = client.call_procedure(
    ///     "##double_it",
    ///     &[
    ///         ProcParam::input("@n", &21i32),
    ///         // The 0 is just a placeholder establishing the type - the
    ///         // procedure overwrites it, we never see this value again.
    ///         ProcParam::output("@doubled", &0i32),
    ///     ],
    /// ).await?;
    ///
    /// let outputs = stream.into_output_params().await?;
    /// assert_eq!(Some(42i32), outputs.get("@doubled"));
    /// assert_eq!(Some(1), outputs.return_status());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn call_procedure<'a, 'b>(
        &'a mut self,
        name: impl Into<Cow<'b, str>>,
        params: &'b [ProcParam<'b>],
    ) -> crate::Result<QueryStream<'a>>
    where
        'a: 'b,
    {
        if let Some(p) = params
            .iter()
            .find(|p| p.output && p.value.is_untyped_null())
        {
            return Err(crate::Error::Conversion(
                format!(
                    "OUTPUT parameter {} has no type: pass a concrete placeholder value \
                     (e.g. &0i32, or a typed NULL like &None::<i32>) - a NULL of this type is \
                     sent untyped, and SQL Server rejects an untyped NULL bound as OUTPUT",
                    p.name
                )
                .into(),
            ));
        }

        self.connection.flush_stream().await?;

        let rpc_params = params
            .iter()
            .map(|p| {
                let mut flags = BitFlags::empty();

                if p.output {
                    flags |= RpcStatus::ByRefValue;
                }

                RpcParam {
                    name: p.name.clone(),
                    flags,
                    value: p.value.clone(),
                    type_info: None,
                    table: None,
                }
            })
            .collect::<Vec<_>>();

        // Table-valued parameters need their table type's column layout,
        // which the server reports for a `DECLARE @t AS <type>` table
        // variable. Fetched before the RPC is sent.
        let mut rpc_params = rpc_params;
        for (param, rpc) in params.iter().zip(rpc_params.iter_mut()) {
            if let Some(table) = &param.table {
                rpc.table = Some(self.table_value(table).await?);
            }
        }

        let req = TokenRpcRequest::new(
            name,
            rpc_params,
            self.connection.context().transaction_descriptor(),
        );

        let id = self.connection.context_mut().next_packet_id();
        self.connection.send(PacketHeader::rpc(id), req).await?;

        let ts = TokenStream::new(&mut self.connection);

        let mut result = QueryStream::new(ts.try_unfold());
        result.forward_to_metadata().await?;

        Ok(result)
    }

    /// Resolves a `ProcParam::table` argument into an encodable TVP by asking
    /// the server for the table type's columns. The type name is a
    /// caller-supplied identifier interpolated into SQL, so it is validated
    /// and each part bracket-quoted. This replaces upstream Tiberius's
    /// unquoted `DECLARE @P AS {db_type}` (73e79d4), which was
    /// SQL-injection-shaped.
    async fn table_value<'p>(
        &mut self,
        table: &crate::proc_param::TableParam<'p>,
    ) -> crate::Result<codec::TableValue<'p>> {
        validate_sql_identifier("table type", &table.type_name)?;
        let parts = split_multipart_identifier(&table.type_name);
        let (schema, type_name) = match parts.as_slice() {
            [t] => (String::new(), t.clone()),
            [s, t] => (s.clone(), t.clone()),
            _ => {
                return Err(crate::Error::Conversion(
                    format!(
                        "table type {:?} must be `type` or `schema.type`",
                        table.type_name
                    )
                    .into(),
                ))
            }
        };
        if type_name.is_empty() {
            return Err(crate::Error::Conversion("empty table type name".into()));
        }

        let qualified = if schema.is_empty() {
            quote_identifier(&type_name)
        } else {
            format!(
                "{}.{}",
                quote_identifier(&schema),
                quote_identifier(&type_name)
            )
        };

        self.connection.flush_stream().await?;
        let query = format!("DECLARE @tvp AS {qualified}; SELECT TOP 0 * FROM @tvp");
        let req = BatchRequest::new(query, self.connection.context().transaction_descriptor());
        let id = self.connection.context_mut().next_packet_id();
        self.connection.send(PacketHeader::batch(id), req).await?;

        let columns = TokenStream::new(&mut self.connection)
            .try_unfold()
            .try_fold(None, |columns, token| async move {
                Ok(match token {
                    ReceivedToken::NewResultset(meta) if columns.is_none() => {
                        Some(meta.columns.clone())
                    }
                    _ => columns,
                })
            })
            .await?
            .ok_or_else(|| {
                crate::Error::Protocol("no column metadata for the table type".into())
            })?;

        let columns = columns
            .into_iter()
            .map(|c| {
                let mut base = c.base;
                base.ty = codec::tvp_column_type(base.ty);
                base
            })
            .collect();

        Ok(codec::TableValue {
            schema,
            type_name,
            columns,
            rows: table.rows.clone(),
        })
    }

    /// Execute a `BULK INSERT` statement, efficiently storing a large number of
    /// rows to a specified table. Note: make sure the input row follows the same
    /// schema as the table, otherwise calling `send()` will return an error.
    ///
    /// This is equivalent to calling `bulk_insert_columns(table, &["*"])` to
    /// merge all of a table's columns.
    ///
    /// # Example
    ///
    /// ```
    /// # use mssql::{Config, IntoRow};
    /// # use tokio_util::compat::TokioAsyncWriteCompatExt;
    /// # use std::env;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let c_str = env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or(
    /// #     "server=tcp:localhost,1433;integratedSecurity=true;TrustServerCertificate=true".to_owned(),
    /// # );
    /// # let config = Config::from_ado_string(&c_str)?;
    /// # let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    /// # tcp.set_nodelay(true)?;
    /// # let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;
    /// let create_table = r#"
    ///     CREATE TABLE ##bulk_test (
    ///         id INT IDENTITY PRIMARY KEY,
    ///         val INT NOT NULL
    ///     )
    /// "#;
    ///
    /// client.simple_query(create_table).await?;
    ///
    /// // Start the bulk insert with the client.
    /// let mut req = client.bulk_insert("##bulk_test").await?;
    ///
    /// for i in [0i32, 1i32, 2i32] {
    ///     let row = (i).into_row();
    ///
    ///     // The request will handle flushing to the wire in an optimal way,
    ///     // balancing between memory usage and IO performance.
    ///     req.send(row).await?;
    /// }
    ///
    /// // The request must be finalized.
    /// let res = req.finalize().await?;
    /// assert_eq!(3, res.total()?);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bulk_insert<'a>(
        &'a mut self,
        table: &'a str,
    ) -> crate::Result<BulkLoadRequest<'a, S>> {
        self.bulk_insert_with_options(table, &["*"], SqlBulkCopyOptions::empty(), &[])
            .await
    }

    /// Retrieve the column metadata for a table (or a subset of its
    /// columns), including each column's name, type, and flags (e.g.
    /// nullability, whether it's an identity or computed column). Runs a
    /// `SELECT TOP 0` internally, so it never touches any row data.
    ///
    /// # Example
    ///
    /// ```
    /// # use mssql::Config;
    /// # use tokio_util::compat::TokioAsyncWriteCompatExt;
    /// # use std::env;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let c_str = env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or(
    /// #     "server=tcp:localhost,1433;integratedSecurity=true;TrustServerCertificate=true".to_owned(),
    /// # );
    /// # let config = Config::from_ado_string(&c_str)?;
    /// # let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    /// # tcp.set_nodelay(true)?;
    /// # let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;
    /// client.simple_query("CREATE TABLE ##describe_test (id INT IDENTITY PRIMARY KEY, name VARCHAR(50) NULL)").await?;
    /// let columns = client.column_metadata("##describe_test", &["*"]).await?;
    /// assert_eq!(2, columns.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn column_metadata<'a>(
        &'a mut self,
        table: &'a str,
        columns: &'a [&'a str],
    ) -> crate::Result<Vec<MetaDataColumn<'static>>> {
        Ok(self.fetch_column_metadata(table, columns, false).await?.0)
    }

    /// Like [`column_metadata`], and with `collations` also the
    /// `(column name, collation name)` of every column of the table, from
    /// `sp_tablecollations_100` run in the same batch (see
    /// [`table_collations_sql`]). Without `collations` that list is empty.
    ///
    /// [`column_metadata`]: #method.column_metadata
    async fn fetch_column_metadata(
        &mut self,
        table: &str,
        columns: &[&str],
        collations: bool,
    ) -> crate::Result<(Vec<MetaDataColumn<'static>>, Vec<(String, Option<String>)>)> {
        // `table` and `columns` are interpolated into the batch below (T-SQL
        // can't parameterize identifiers). Reject input that can't be a
        // legitimate identifier before building any SQL. This is
        // defense-in-depth, not a way to make untrusted input safe. Every
        // bulk-insert entry point and `column_metadata` come through here.
        validate_sql_identifier("table", table)?;
        for column in columns {
            validate_sql_identifier("column", column)?;
        }

        self.connection.flush_stream().await?;

        let columns_list = columns.join(", ");
        let mut query = format!("SELECT TOP 0 {columns_list} FROM {table}");
        if collations {
            let version = self.connection.context().version();
            query.push_str("; ");
            query.push_str(&table_collations_sql(table, version));
        }

        let req = BatchRequest::new(query, self.connection.context().transaction_descriptor());

        let id = self.connection.context_mut().next_packet_id();
        self.connection.send(PacketHeader::batch(id), req).await?;

        let token_stream = TokenStream::new(&mut self.connection).try_unfold();

        // The `SELECT TOP 0` result set comes first and has no rows; every
        // row is one of the collation list (`colid`, `name`,
        // `tds_collation_100`, `collation_100`).
        let (columns, collations) = token_stream
            .try_fold(
                (None, Vec::new()),
                |(mut columns, mut collations), token| async move {
                    match token {
                        ReceivedToken::NewResultset(metadata) if columns.is_none() => {
                            columns = Some(metadata.columns.clone());
                        }
                        ReceivedToken::Row(row) => {
                            let string = |i| match row.get(i) {
                                Some(ColumnData::String(s)) => s.as_ref().map(|s| s.to_string()),
                                _ => None,
                            };
                            if let Some(name) = string(1) {
                                collations.push((name, string(3)));
                            }
                        }
                        _ => {}
                    }

                    Ok((columns, collations))
                },
            )
            .await?;

        let columns = columns.ok_or_else(|| {
            crate::Error::Protocol("expecting column metadata from query but not found".into())
        })?;

        Ok((columns, collations))
    }

    /// Execute a `BULK INSERT` statement, efficiently storing a large number of
    /// rows into a specified list of a table's columns. Note: make sure the
    /// input row follows the same schema as the column list, otherwise
    /// calling `send()` will return an error.
    ///
    /// # Example
    ///
    /// ```
    /// # use mssql::{Config, IntoRow};
    /// # use tokio_util::compat::TokioAsyncWriteCompatExt;
    /// # use std::env;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let c_str = env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or(
    /// #     "server=tcp:localhost,1433;integratedSecurity=true;TrustServerCertificate=true".to_owned(),
    /// # );
    /// # let config = Config::from_ado_string(&c_str)?;
    /// # let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    /// # tcp.set_nodelay(true)?;
    /// # let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;
    /// let create_table = r#"
    ///     CREATE TABLE ##bulk_test_columns (
    ///         id INT IDENTITY PRIMARY KEY,
    ///         foo INT NOT NULL,
    ///         bar FLOAT NOT NULL
    ///     )
    /// "#;
    ///
    /// client.simple_query(create_table).await?;
    ///
    /// // Start the bulk insert with the client.
    /// let mut req = client.bulk_insert_columns("##bulk_test_columns", &["foo", "bar"]).await?;
    ///
    /// for (i, j) in [(0i32, 0f64), (1i32, 1f64), (2i32, 2f64)] {
    ///     let row = (i, j).into_row();
    ///
    ///     // The request will handle flushing to the wire in an optimal way,
    ///     // balancing between memory usage and IO performance.
    ///     req.send(row).await?;
    /// }
    ///
    /// // The request must be finalized.
    /// let res = req.finalize().await?;
    /// assert_eq!(3, res.total()?);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bulk_insert_columns<'a>(
        &'a mut self,
        table: &'a str,
        columns: &'a [&'a str],
    ) -> crate::Result<BulkLoadRequest<'a, S>> {
        self.bulk_insert_with_options(table, columns, SqlBulkCopyOptions::empty(), &[])
            .await
    }

    /// Execute a `BULK INSERT` statement with fine-grained control over its
    /// `INSERT BULK ... WITH (...)` options and `ORDER` hint, mirroring
    /// .NET's [`SqlBulkCopy`](https://learn.microsoft.com/en-us/dotnet/api/system.data.sqlclient.sqlbulkcopy).
    /// `bulk_insert` and `bulk_insert_columns` are convenience wrappers
    /// around this method with no options and no order hints.
    ///
    /// # Example
    ///
    /// ```
    /// # use mssql::{Config, IntoRow, SqlBulkCopyOption, SortOrder};
    /// # use tokio_util::compat::TokioAsyncWriteCompatExt;
    /// # use std::env;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let c_str = env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or(
    /// #     "server=tcp:localhost,1433;integratedSecurity=true;TrustServerCertificate=true".to_owned(),
    /// # );
    /// # let config = Config::from_ado_string(&c_str)?;
    /// # let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    /// # tcp.set_nodelay(true)?;
    /// # let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;
    /// client.simple_query("CREATE TABLE ##bulk_opts_test (val INT NOT NULL)").await?;
    ///
    /// let options = SqlBulkCopyOption::TableLock | SqlBulkCopyOption::FireTriggers;
    /// let order_hints = [("val", SortOrder::Ascending)];
    /// let mut req = client
    ///     .bulk_insert_with_options("##bulk_opts_test", &["*"], options, &order_hints)
    ///     .await?;
    ///
    /// for i in [0i32, 1i32, 2i32] {
    ///     req.send(i.into_row()).await?;
    /// }
    ///
    /// let res = req.finalize().await?;
    /// assert_eq!(3, res.total()?);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bulk_insert_with_options<'a>(
        &'a mut self,
        table: &'a str,
        columns: &'a [&'a str],
        options: SqlBulkCopyOptions,
        order_hints: &'a [ColumnOrderHint<'a>],
    ) -> crate::Result<BulkLoadRequest<'a, S>> {
        let keep_identity = options.contains(SqlBulkCopyOption::KeepIdentity);

        // The same batch lists every column's collation, which the INSERT
        // BULK column list declares for character columns (see
        // `bulk_column_list`).
        let (columns, collations) = self.fetch_column_metadata(table, columns, true).await?;
        let columns: Vec<_> = columns
            .into_iter()
            // `usUpdateable` is a 2-bit value (0 = read-only, 1 = read/write,
            // 2 = unknown), not two independent flags: keep the column
            // unless the server has positively marked it read-only (e.g.
            // identity/computed columns). Checking `Updateable` alone would
            // wrongly exclude ordinary columns, since servers commonly
            // report plain `SELECT` columns as updateable-unknown rather
            // than definitively read/write. `KeepIdentity` overrides this
            // for identity columns specifically, mirroring how BULK
            // INSERT/SqlBulkCopy's own KeepIdentity works: by including the
            // identity column in the column list instead of relying on a
            // WITH-clause keyword (the TDS bulk-load protocol has none).
            .filter(|column| {
                let flags = column.base.flags;

                flags.intersects(ColumnFlag::Updateable | ColumnFlag::UpdateableUnknown)
                    || (keep_identity && flags.contains(ColumnFlag::Identity))
            })
            .collect();

        // now start bulk upload
        self.connection.flush_stream().await?;
        let col_data = bulk_column_list(&columns, &collations)?;
        let mut query = format!("INSERT BULK {} ({})", table, col_data);

        // Note: `KeepIdentity` never contributes a hint here - it's handled
        // entirely by the column filter above, since the TDS bulk-load
        // protocol has no WITH-clause keyword for it. So `hints` can end up
        // empty even when `options` isn't, and the WITH(...) clause must
        // only be emitted when there's actually something to put in it -
        // `WITH ()` is invalid syntax.
        let mut hints = Vec::with_capacity(4);

        if options.contains(SqlBulkCopyOption::CheckConstraints) {
            hints.push("CHECK_CONSTRAINTS".to_owned());
        }
        if options.contains(SqlBulkCopyOption::FireTriggers) {
            hints.push("FIRE_TRIGGERS".to_owned());
        }
        if options.contains(SqlBulkCopyOption::KeepNulls) {
            hints.push("KEEP_NULLS".to_owned());
        }
        if options.contains(SqlBulkCopyOption::TableLock) {
            hints.push("TABLOCK".to_owned());
        }
        if !order_hints.is_empty() {
            let order = order_hints
                .iter()
                .map(|(col, order)| {
                    let dir = match order {
                        SortOrder::Ascending => "ASC",
                        SortOrder::Descending => "DESC",
                    };

                    // Escape `]` the same way column names are escaped
                    // elsewhere, so an ORDER hint can't break out of the
                    // bracket-quoted identifier.
                    format!("[{}] {dir}", col.replace(']', "]]"))
                })
                .join(", ");

            hints.push(format!("ORDER ({order})"));
        }

        if !hints.is_empty() {
            query.push_str(" WITH (");
            query.push_str(&hints.join(", "));
            query.push(')');
        }

        let req = BatchRequest::new(query, self.connection.context().transaction_descriptor());
        let id = self.connection.context_mut().next_packet_id();

        self.connection.send(PacketHeader::batch(id), req).await?;

        let ts = TokenStream::new(&mut self.connection);
        ts.flush_done().await?;

        BulkLoadRequest::new(&mut self.connection, columns, table)
    }

    /// Closes this database connection explicitly.
    pub async fn close(self) -> crate::Result<()> {
        self.connection.close().await
    }

    /// Begins a transaction with a TDS Transaction Manager request
    /// (`TM_BEGIN_XACT`, MS-TDS 2.2.6.9) instead of a `BEGIN TRAN` batch.
    /// The server's transaction descriptor (from its ENVCHANGE reply) is
    /// attached to every following request until
    /// [`commit_transaction`](Self::commit_transaction) or
    /// [`rollback_transaction`](Self::rollback_transaction). Uses the
    /// session's isolation level; see
    /// [`begin_transaction_with_isolation`](Self::begin_transaction_with_isolation).
    ///
    /// ```
    /// # use mssql::{Config, IsolationLevel};
    /// # use tokio_util::compat::TokioAsyncWriteCompatExt;
    /// # use std::env;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let c_str = env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or(
    /// #     "server=tcp:localhost,1433;integratedSecurity=true;TrustServerCertificate=true".to_owned(),
    /// # );
    /// # let config = Config::from_ado_string(&c_str)?;
    /// # let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    /// # let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;
    /// client.begin_transaction_with_isolation(IsolationLevel::Serializable).await?;
    /// client.execute("SELECT 1", &[]).await?;
    /// client.commit_transaction().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Ported from upstream Tiberius (tiberius-rs/tiberius 8338bb8).
    pub async fn begin_transaction(&mut self) -> crate::Result<()> {
        self.begin_transaction_with_isolation(IsolationLevel::Unspecified)
            .await
    }

    /// [`begin_transaction`](Self::begin_transaction) with an explicit
    /// isolation level.
    pub async fn begin_transaction_with_isolation(
        &mut self,
        isolation_level: IsolationLevel,
    ) -> crate::Result<()> {
        let desc = self.connection.context().transaction_descriptor();
        self.send_transaction_manager_request(TransactionManagerRequest::begin(
            desc,
            isolation_level,
            "".into(),
        ))
        .await
    }

    /// Commits the active transaction (`TM_COMMIT_XACT`).
    pub async fn commit_transaction(&mut self) -> crate::Result<()> {
        let desc = self.connection.context().transaction_descriptor();
        self.send_transaction_manager_request(TransactionManagerRequest::commit(desc, "".into()))
            .await
    }

    /// Rolls back the active transaction (`TM_ROLLBACK_XACT`).
    pub async fn rollback_transaction(&mut self) -> crate::Result<()> {
        let desc = self.connection.context().transaction_descriptor();
        self.send_transaction_manager_request(TransactionManagerRequest::rollback(desc, "".into()))
            .await
    }

    /// Rolls back to a savepoint created with
    /// [`save_transaction`](Self::save_transaction), keeping the outer
    /// transaction open (`TM_ROLLBACK_XACT` with a name).
    pub async fn rollback_to_savepoint<'n>(
        &mut self,
        name: impl Into<Cow<'n, str>>,
    ) -> crate::Result<()> {
        let desc = self.connection.context().transaction_descriptor();
        self.send_transaction_manager_request(TransactionManagerRequest::rollback(
            desc,
            name.into(),
        ))
        .await
    }

    /// Creates a named savepoint in the active transaction (`TM_SAVE_XACT`).
    pub async fn save_transaction<'n>(
        &mut self,
        name: impl Into<Cow<'n, str>>,
    ) -> crate::Result<()> {
        let desc = self.connection.context().transaction_descriptor();
        self.send_transaction_manager_request(TransactionManagerRequest::save(desc, name.into()))
            .await
    }

    async fn send_transaction_manager_request(
        &mut self,
        req: TransactionManagerRequest<'_>,
    ) -> crate::Result<()> {
        self.connection.flush_stream().await?;

        let id = self.connection.context_mut().next_packet_id();
        self.connection
            .send(PacketHeader::transaction_manager(id), req)
            .await?;

        // DONE, plus an ENVCHANGE that the token stream applies to the
        // context's transaction descriptor.
        TokenStream::new(&mut self.connection).flush_done().await?;
        Ok(())
    }

    pub(crate) fn rpc_params<'a>(query: impl Into<Cow<'a, str>>) -> Vec<RpcParam<'a>> {
        vec![
            RpcParam {
                name: Cow::Borrowed("stmt"),
                flags: BitFlags::empty(),
                value: ColumnData::String(Some(query.into())),
                type_info: None,
                table: None,
            },
            RpcParam {
                name: Cow::Borrowed("params"),
                flags: BitFlags::empty(),
                value: ColumnData::I32(Some(0)),
                type_info: None,
                table: None,
            },
        ]
    }

    /// A collation covering the ASCII range, used as the default when
    /// [`Config::send_string_parameters_as_unicode`] is disabled and no
    /// column-specific collation is otherwise available. Corresponds to
    /// LCID `0x0409` (English - United States), matching SQL Server's most
    /// common installation default (`SQL_Latin1_General_CP1_CI_AS`).
    fn default_varchar_collation() -> Collation {
        Collation::new(0x0409, 0)
    }

    /// Type info for sending a string parameter as `VARCHAR(MAX)` instead of
    /// the default `NVARCHAR(MAX)`, using [`default_varchar_collation`].
    ///
    /// [`default_varchar_collation`]: #method.default_varchar_collation
    fn varchar_type_info() -> TypeInfo {
        TypeInfo::VarLenSized(VarLenContext::new(
            VarLenType::BigVarChar,
            0xffff_ffff,
            Some(Self::default_varchar_collation()),
        ))
    }

    pub(crate) async fn rpc_perform_query<'a, 'b>(
        &'a mut self,
        proc_id: RpcProcId,
        mut rpc_params: Vec<RpcParam<'b>>,
        params: impl Iterator<Item = ColumnData<'b>>,
    ) -> crate::Result<()>
    where
        'a: 'b,
    {
        let unicode = self
            .connection
            .context()
            .send_string_parameters_as_unicode();
        let mut param_str = String::new();

        for (i, param) in params.enumerate() {
            // SQL Server has no documented wire encoding for a NaN or
            // infinite float parameter - sending one round-trips to a
            // cryptic server-side error 8023. Reject it client-side with a
            // clear message instead (#221).
            match param {
                ColumnData::F32(Some(f)) if !f.is_finite() => {
                    return Err(crate::Error::Conversion(
                        format!("cannot send non-finite f32 value {f} as a query parameter").into(),
                    ));
                }
                ColumnData::F64(Some(f)) if !f.is_finite() => {
                    return Err(crate::Error::Conversion(
                        format!("cannot send non-finite f64 value {f} as a query parameter").into(),
                    ));
                }
                _ => {}
            }

            if i > 0 {
                param_str.push(',')
            }
            param_str.push_str(&format!("@P{} ", i + 1));

            let type_info = if !unicode && matches!(param, ColumnData::String(Some(_))) {
                param_str.push_str("varchar(max)");
                Some(Self::varchar_type_info())
            } else {
                param_str.push_str(&param.type_name());
                None
            };

            rpc_params.push(RpcParam {
                name: Cow::Owned(format!("@P{}", i + 1)),
                flags: BitFlags::empty(),
                value: param,
                type_info,
                table: None,
            });
        }

        if let Some(params) = rpc_params.iter_mut().find(|x| x.name == "params") {
            params.value = ColumnData::String(Some(param_str.into()));
        }

        let req = TokenRpcRequest::new(
            proc_id,
            rpc_params,
            self.connection.context().transaction_descriptor(),
        );

        let id = self.connection.context_mut().next_packet_id();
        self.connection.send(PacketHeader::rpc(id), req).await?;

        Ok(())
    }
}

/// Build the `INSERT BULK` column list: each column bracket-quoted with its
/// type, and every character column followed by ` COLLATE <name>`.
///
/// SQL Server reads the bulk data of a column as the type declared here. A
/// char/varchar/text declaration without `COLLATE` takes the database default
/// collation, so the server read the bytes in that code page and converted
/// them to the column's, silently corrupting text bulk-loaded into a column
/// whose collation differs from the database default. Like SqlClient's
/// `SqlBulkCopy`, each character column's own collation is now declared.
/// Ported from upstream Tiberius (tiberius-rs/tiberius 75d7afc).
///
/// `MetaDataColumn`'s Display returns `fmt::Error` for a column type INSERT
/// BULK can't declare; this maps that to `Error::BulkInput`, where `format!`
/// would panic.
fn bulk_column_list(
    columns: &[MetaDataColumn<'_>],
    collations: &[(String, Option<String>)],
) -> crate::Result<String> {
    let mut list = Vec::with_capacity(columns.len());

    for column in columns {
        let mut item = String::new();
        std::fmt::Write::write_fmt(&mut item, format_args!("{column}")).map_err(|_| {
            crate::Error::BulkInput(
                format!(
                    "column {} has a type bulk insert cannot declare: {:?}",
                    column.col_name, column.base.ty
                )
                .into(),
            )
        })?;

        if declares_collation(&column.base.ty) {
            if let Some(collation) = column_collation(&column.col_name, collations) {
                validate_collation_name(collation)?;
                item.push_str(" COLLATE ");
                item.push_str(collation);
            }
        }

        list.push(item);
    }

    Ok(list.join(", "))
}

/// Whether the `INSERT BULK` declaration of a column of type `ty` carries a
/// `COLLATE` clause: the character types SqlClient declares one for.
fn declares_collation(ty: &TypeInfo) -> bool {
    matches!(
        ty,
        TypeInfo::VarLenSized(ctx) if matches!(
            ctx.r#type(),
            VarLenType::BigChar
                | VarLenType::BigVarChar
                | VarLenType::Text
                | VarLenType::NChar
                | VarLenType::NVarchar
                | VarLenType::NText
        )
    )
}

/// The collation name listed for the column `name`: the row with exactly this
/// name, else the only row whose name matches ignoring case.
fn column_collation<'a>(name: &str, collations: &'a [(String, Option<String>)]) -> Option<&'a str> {
    let row = match collations.iter().find(|(n, _)| n == name) {
        Some(row) => row,
        None => {
            let name = name.to_lowercase();
            let mut rows = collations.iter().filter(|(n, _)| n.to_lowercase() == name);
            match (rows.next(), rows.next()) {
                (Some(row), None) => row,
                _ => return None,
            }
        }
    };

    row.1.as_deref()
}

/// The batch statement listing the collation of every column of `table`, as
/// SqlClient's `SqlBulkCopy` runs it:
/// `EXEC <catalog>..sp_tablecollations_100 N'<schema>.<table>'`.
///
/// The procedure reads the catalog it is called in: the catalog part of
/// `table`, else `tempdb` for a `#` temp table, else the current database.
/// Schema and table are bracket-quoted inside an N'' literal with `'`
/// doubled, and the catalog is bracket-quoted, so a caller-supplied name
/// cannot break out of either. Before SQL Server 2008 the procedure is
/// `sp_tablecollations_90`.
fn table_collations_sql(table: &str, version: codec::FeatureLevel) -> String {
    let parts = split_multipart_identifier(table);
    let part = |from_end: usize| {
        parts
            .len()
            .checked_sub(from_end + 1)
            .map_or("", |i| parts[i].as_str())
    };

    let table_name = part(0);
    let schema = part(1);
    let catalog = part(2);

    let catalog = if catalog.is_empty() {
        if table_name.starts_with('#') {
            "tempdb".to_owned()
        } else {
            String::new()
        }
    } else {
        quote_identifier(catalog)
    };

    let literal = |part: &str| {
        if part.is_empty() {
            String::new()
        } else {
            quote_identifier(&part.replace('\'', "''"))
        }
    };

    let procedure = if version >= codec::FeatureLevel::SqlServer2008 {
        "sp_tablecollations_100"
    } else {
        "sp_tablecollations_90"
    };

    format!(
        "EXEC {catalog}..{procedure} N'{}.{}'",
        literal(schema),
        literal(table_name)
    )
}

/// Reject an obviously-malformed or dangerous SQL identifier that will be
/// interpolated directly into a SQL batch because T-SQL does not allow
/// identifiers or type names to be parameterized. `what` names the kind of
/// identifier for the error message.
///
/// This is cheap defense-in-depth — it does NOT make untrusted input safe. It
/// rejects input that cannot be a legitimate identifier:
///
/// - a NUL byte or any ASCII control character;
/// - **outside** a `[...]` bracket-quoted segment, any character that is not
///   identifier-safe. Only alphanumerics and the punctuation needed for real
///   identifiers/type names are allowed — `_`, `.` (multi-part names like
///   `dbo.MyType`), `@`/`#` (variable/temp-style names), `*` (a bulk column
///   list may be `*`), and `(`, `)`, `,` (parameterized types such as
///   `decimal(10,2)` / `varchar(max)`). Parens must be balanced — an unbalanced
///   `)` *or* an unclosed `(` is rejected — and both a space and a comma are allowed only *inside*
///   those parens (e.g. `decimal(18, 4)`); a top-level space or comma is
///   rejected because it would let one identifier split into several SQL tokens
///   (a top-level comma would splice a verbatim order hint `a,b` into two
///   columns). This rejects statement-breaking characters such as `;`, quotes
///   and `-`, so a value like `1; DROP TABLE Users--` cannot slip through; and
/// - **inside** a `[...]` bracket-quoted segment anything is allowed except an
///   unescaped `]` (per the T-SQL bracket-escaping rule a literal `]` must be
///   doubled as `]]`). The segment must be terminated — an unclosed `[` running
///   to the end of the string is rejected. A `]` seen outside any bracket is
///   unbalanced and rejected. A closing `]`, and a top-level closing `)`, are themselves SQL
///   token boundaries, so a segment may only be followed by `.` (the next part
///   of a multi-part name) or the end of the string — this stops delimiter-
///   adjacent splicing such as `[t]UNION(...)` or `foo(1)UNION(...)` that needs
///   no space.
///
/// It deliberately does NOT try to quote or rewrite the identifier, so
/// multi-part names (`schema.table`), already-bracketed names (`[my table]`,
/// `[dbo].[my table]`, `[weird]]name]`) and temp tables (`##bulk_test`) keep
/// working unchanged.
///
/// Ported from upstream Tiberius (tiberius-rs/tiberius 7a86677 / 3f47d87).
/// Fork change: `$` is also allowed, since T-SQL identifiers may contain it
/// (e.g. `t$1`).
fn validate_sql_identifier(what: &str, ident: &str) -> crate::Result<()> {
    if ident.chars().any(|c| c.is_ascii_control()) {
        return Err(crate::Error::BulkInput(
            format!("{what} identifier must not contain NUL or control characters").into(),
        ));
    }

    // Apply the T-SQL bracket rule: inside a `[...]` quoted identifier a literal
    // `]` must be doubled (`]]`); a single `]` closes the bracket. A `]` seen
    // outside of any bracket is unbalanced and rejected. Tracking bracket state
    // keeps legitimate names like `[dbo].[my table]` and `[weird]]name]`
    // working while catching stray closing brackets such as `Foo]`.
    let mut chars = ident.chars().peekable();
    let mut in_bracket = false;
    let mut paren_depth: u32 = 0;
    while let Some(c) = chars.next() {
        if in_bracket {
            if c == ']' {
                if chars.peek() == Some(&']') {
                    chars.next(); // consume the doubled `]]` escape
                } else {
                    in_bracket = false;
                    // A quoted-identifier segment may only be followed by `.`
                    // (introducing the next part of a multi-part name) or the
                    // end of the string. Anything else — e.g. `[t]UNION ...` —
                    // splices a fresh SQL token directly onto the closing `]`
                    // (which is itself a token boundary, needing no space), so
                    // reject it.
                    match chars.peek() {
                        None | Some('.') => {}
                        Some(_) => {
                            return Err(crate::Error::BulkInput(
                                format!("{what} identifier has trailing characters after a bracket-quoted segment").into(),
                            ));
                        }
                    }
                }
            }
            continue;
        }

        match c {
            '[' => in_bracket = true,
            ']' => {
                return Err(crate::Error::BulkInput(
                    format!("{what} identifier contains an unbalanced `]` bracket").into(),
                ));
            }
            '(' => paren_depth += 1,
            ')' => {
                // A `)` with no matching `(` is unbalanced. Left unchecked
                // (`saturating_sub`) it would sail through and, interpolated
                // verbatim, could close a caller-supplied enclosing paren early
                // — e.g. an order hint `col)` desyncing `ORDER(col) ...)`. Reject
                // it exactly like an unbalanced `]`.
                if paren_depth == 0 {
                    return Err(crate::Error::BulkInput(
                        format!("{what} identifier contains an unbalanced `)`").into(),
                    ));
                }
                paren_depth -= 1;
                // A parameterized type ends at its closing paren; nothing is a
                // legitimate continuation after a top-level `)`. Rejecting it
                // stops `)`-delimited token splicing such as `foo(1)UNION(...)`
                // (the `)` is a token boundary, so no space is required).
                if paren_depth == 0 && chars.peek().is_some() {
                    return Err(crate::Error::BulkInput(
                        format!("{what} identifier has trailing characters after a closing `)`")
                            .into(),
                    ));
                }
            }
            // A space is only legitimate inside a parameterized type's parens
            // (e.g. `decimal(18, 4)`) or inside a bracket-quoted name (handled
            // above). A *top-level* space would let a single identifier split
            // into several SQL tokens (`t UNION SELECT ...`, `t WHERE ...`)
            // without using any otherwise-blocked character, so reject it.
            ' ' if paren_depth == 0 => {
                return Err(crate::Error::BulkInput(
                    format!("{what} identifier contains a disallowed character").into(),
                ));
            }
            // A comma is only legitimate inside a parameterized type's parens
            // (`decimal(10,2)`). A *top-level* comma would splice a single
            // identifier into two — e.g. a verbatim order hint `a,b` becoming
            // two order columns `a` and `b` — so reject it like a top-level
            // space.
            ',' if paren_depth == 0 => {
                return Err(crate::Error::BulkInput(
                    format!("{what} identifier contains a disallowed character").into(),
                ));
            }
            // Outside a bracket-quoted segment only identifier-safe characters
            // are permitted (see the doc comment). Everything else — `;`,
            // quotes, `-`, etc. — is rejected.
            c if c.is_alphanumeric() => {}
            '_' | '.' | '@' | '#' | '$' | ',' | ' ' | '*' => {}
            _ => {
                return Err(crate::Error::BulkInput(
                    format!("{what} identifier contains a disallowed character").into(),
                ));
            }
        }
    }

    // Reject unbalanced *openers* left dangling at end of input, mirroring the
    // rejection of unbalanced closers above. An unterminated `[` would quote and
    // swallow whatever follows when interpolated (e.g. an order hint `[abc`
    // becoming `ORDER([abc ASC))`); an unclosed `(` leaves an enclosing paren
    // open. A legitimate identifier/type name never ends mid-bracket or with an
    // open paren.
    if in_bracket {
        return Err(crate::Error::BulkInput(
            format!("{what} identifier has an unterminated `[` bracket-quoted segment").into(),
        ));
    }
    if paren_depth != 0 {
        return Err(crate::Error::BulkInput(
            format!("{what} identifier contains an unbalanced `(`").into(),
        ));
    }

    Ok(())
}

/// Bracket-quote `name`, doubling any `]` in it.
fn quote_identifier(name: &str) -> String {
    format!("[{}]", name.replace(']', "]]"))
}

/// Split a multi-part identifier (`server.catalog.schema.table`, any leading
/// parts omitted or empty) into its unquoted parts: `[a]]b].c` gives `a]b`
/// and `c`. An unterminated bracket simply runs to the end.
fn split_multipart_identifier(ident: &str) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut chars = ident.chars().peekable();
    let mut in_bracket = false;

    while let Some(c) = chars.next() {
        let part = parts.last_mut().expect("parts is never empty");

        if in_bracket {
            if c == ']' {
                if chars.peek() == Some(&']') {
                    chars.next();
                    part.push(']');
                } else {
                    in_bracket = false;
                }
            } else {
                part.push(c);
            }
        } else {
            match c {
                '[' => in_bracket = true,
                '.' => parts.push(String::new()),
                c => part.push(c),
            }
        }
    }

    parts
}

/// Reject a server-reported collation name that is not a plain identifier of
/// ASCII letters, digits and `_`, as every SQL Server collation name is: it is
/// put into the `INSERT BULK` statement unquoted.
fn validate_collation_name(name: &str) -> crate::Result<()> {
    if !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        Ok(())
    } else {
        Err(crate::Error::Protocol(
            format!("invalid collation name {name:?} for a bulk insert column").into(),
        ))
    }
}

// Ported from upstream Tiberius (tiberius-rs/tiberius 75d7afc).
// Server-free tests for the column collations declared in `INSERT BULK`.
#[cfg(test)]
mod bulk_collation_tests {
    use super::{bulk_column_list, table_collations_sql, validate_collation_name, Client};
    use crate::tds::codec::FeatureLevel;
    use crate::tds::codec::{
        BaseMetaDataColumn, ColumnFlag, FixedLenType, TypeInfo, VarLenContext, VarLenType,
    };
    use crate::tds::Collation;
    use crate::MetaDataColumn;
    use std::borrow::Cow;

    fn column(name: &'static str, ty: TypeInfo) -> MetaDataColumn<'static> {
        MetaDataColumn {
            base: BaseMetaDataColumn {
                flags: ColumnFlag::Updateable.into(),
                ty,
            },
            col_name: Cow::Borrowed(name),
        }
    }

    fn var(ty: VarLenType, len: usize) -> TypeInfo {
        // Latin1_General_CI_AS; the declared name comes from the server's
        // collation list, not from these bytes.
        let collation = match ty {
            VarLenType::BigChar
            | VarLenType::BigVarChar
            | VarLenType::Text
            | VarLenType::NChar
            | VarLenType::NVarchar
            | VarLenType::NText => Some(Collation::new(0x00d0_0409, 52)),
            _ => None,
        };
        TypeInfo::VarLenSized(VarLenContext::new(ty, len, collation))
    }

    fn collations(rows: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
        rows.iter()
            .map(|(name, collation)| (name.to_string(), collation.map(str::to_string)))
            .collect()
    }

    #[test]
    fn default_non_unicode_parameter_type_is_varchar_max_with_us_english_collation() {
        type MemoryClient = Client<futures_util::io::Cursor<Vec<u8>>>;
        let collation = MemoryClient::default_varchar_collation();
        assert_eq!(0x0409, collation.lcid());

        let TypeInfo::VarLenSized(context) = MemoryClient::varchar_type_info() else {
            panic!("VARCHAR type info must be variable length");
        };
        assert_eq!(VarLenType::BigVarChar, context.r#type());
        assert_eq!(0xffff_ffff, context.len());
        assert_eq!(Some(collation), context.collation());
    }

    #[test]
    fn character_columns_declare_their_collation() {
        let columns = [
            column("id", TypeInfo::FixedLen(FixedLenType::Int4)),
            column("c", var(VarLenType::BigChar, 6)),
            column("v", var(VarLenType::BigVarChar, 20)),
            column("vmax", var(VarLenType::BigVarChar, 0xffff)),
            column("t", var(VarLenType::Text, 0x7fff_ffff)),
            column("nc", var(VarLenType::NChar, 12)),
            column("nv", var(VarLenType::NVarchar, 40)),
            column("nt", var(VarLenType::NText, 0x7fff_ffff)),
            column("b", var(VarLenType::BigVarBin, 16)),
            column("img", var(VarLenType::Image, 0x7fff_ffff)),
        ];
        let rows = collations(&[
            ("id", None),
            ("c", Some("Cyrillic_General_CI_AS")),
            ("v", Some("Cyrillic_General_CI_AS")),
            ("vmax", Some("Chinese_PRC_CI_AS")),
            ("t", Some("Cyrillic_General_CI_AS")),
            ("nc", Some("Latin1_General_100_CI_AS_SC")),
            ("nv", Some("Latin1_General_100_CI_AS_SC")),
            ("nt", Some("Latin1_General_100_CI_AS_SC")),
            ("b", None),
            ("img", None),
        ]);

        assert_eq!(
            bulk_column_list(&columns, &rows).unwrap(),
            "[id] int, \
             [c] char(6) COLLATE Cyrillic_General_CI_AS, \
             [v] varchar(20) COLLATE Cyrillic_General_CI_AS, \
             [vmax] varchar(max) COLLATE Chinese_PRC_CI_AS, \
             [t] text COLLATE Cyrillic_General_CI_AS, \
             [nc] nchar(12) COLLATE Latin1_General_100_CI_AS_SC, \
             [nv] nvarchar(40) COLLATE Latin1_General_100_CI_AS_SC, \
             [nt] ntext COLLATE Latin1_General_100_CI_AS_SC, \
             [b] varbinary(16), \
             [img] image"
        );
    }

    #[test]
    fn non_character_columns_never_declare_a_collation() {
        // Even if the server listed a collation for them.
        let columns = [
            column("id", TypeInfo::FixedLen(FixedLenType::Int4)),
            column("b", var(VarLenType::BigVarBin, 16)),
        ];
        let rows = collations(&[
            ("id", Some("Latin1_General_CI_AS")),
            ("b", Some("Latin1_General_CI_AS")),
        ]);

        assert_eq!(
            bulk_column_list(&columns, &rows).unwrap(),
            "[id] int, [b] varbinary(16)"
        );
    }

    #[test]
    fn undeclarable_bulk_column_types_return_a_typed_error() {
        let columns = [column(
            "unsupported",
            TypeInfo::FixedLen(FixedLenType::Null),
        )];
        let error = bulk_column_list(&columns, &[]).unwrap_err();
        assert!(matches!(error, crate::Error::BulkInput(_)));
        assert!(error.to_string().contains("unsupported"));
    }

    #[test]
    fn collations_are_matched_by_column_name() {
        let columns = [
            column("B", var(VarLenType::BigVarChar, 10)),
            column("a", var(VarLenType::BigVarChar, 10)),
            column("missing", var(VarLenType::BigVarChar, 10)),
            column("no_collation", var(VarLenType::BigVarChar, 10)),
        ];
        // Out of order, a case-insensitive match for `B`, an exact match for
        // `a` that wins over `A`, no row for `missing` and a NULL collation.
        let rows = collations(&[
            ("A", Some("Greek_CI_AS")),
            ("a", Some("Cyrillic_General_CI_AS")),
            ("b", Some("Hebrew_CI_AS")),
            ("no_collation", None),
        ]);

        assert_eq!(
            bulk_column_list(&columns, &rows).unwrap(),
            "[B] varchar(10) COLLATE Hebrew_CI_AS, \
             [a] varchar(10) COLLATE Cyrillic_General_CI_AS, \
             [missing] varchar(10), \
             [no_collation] varchar(10)"
        );
    }

    #[test]
    fn bad_collation_names_are_rejected() {
        for name in [
            "",
            "Latin1_General_CI_AS; DROP TABLE t",
            "Latin1 General",
            "Latin1_General_CI_AS'",
            "[Latin1_General_CI_AS]",
            "Latin1-General",
            "Latin1_General_CI_AS\0",
            "Кириллица",
        ] {
            assert!(validate_collation_name(name).is_err(), "{name:?}");
        }
        for name in [
            "Latin1_General_CI_AS",
            "SQL_Latin1_General_CP1_CI_AS",
            "Latin1_General_100_CI_AS_SC_UTF8",
        ] {
            assert!(validate_collation_name(name).is_ok(), "{name:?}");
        }

        let columns = [column("v", var(VarLenType::BigVarChar, 10))];
        let rows = collations(&[("v", Some("Latin1_General_CI_AS) --"))]);
        assert!(matches!(
            bulk_column_list(&columns, &rows),
            Err(crate::Error::Protocol(_))
        ));
    }

    #[test]
    fn table_collations_query_matches_sqlclient() {
        let v = FeatureLevel::SqlServerN;
        assert_eq!(
            table_collations_sql("t", v),
            "EXEC ..sp_tablecollations_100 N'.[t]'"
        );
        assert_eq!(
            table_collations_sql("dbo.t", v),
            "EXEC ..sp_tablecollations_100 N'[dbo].[t]'"
        );
        assert_eq!(
            table_collations_sql("[my db].[dbo].[it's]]x]", v),
            "EXEC [my db]..sp_tablecollations_100 N'[dbo].[it''s]]x]'"
        );
        assert_eq!(
            table_collations_sql("srv.db.s.t", v),
            "EXEC [db]..sp_tablecollations_100 N'[s].[t]'"
        );
        assert_eq!(
            table_collations_sql("db..t", v),
            "EXEC [db]..sp_tablecollations_100 N'.[t]'"
        );
    }

    #[test]
    fn table_collations_query_reads_temp_tables_from_tempdb() {
        let v = FeatureLevel::SqlServerN;
        assert_eq!(
            table_collations_sql("#t", v),
            "EXEC tempdb..sp_tablecollations_100 N'.[#t]'"
        );
        assert_eq!(
            table_collations_sql("##t", v),
            "EXEC tempdb..sp_tablecollations_100 N'.[##t]'"
        );
        assert_eq!(
            table_collations_sql("[#t]", v),
            "EXEC tempdb..sp_tablecollations_100 N'.[#t]'"
        );
        assert_eq!(
            table_collations_sql("tempdb..#t", v),
            "EXEC [tempdb]..sp_tablecollations_100 N'.[#t]'"
        );
    }

    #[test]
    fn table_collations_query_uses_the_2005_procedure_before_2008() {
        assert_eq!(
            table_collations_sql("t", FeatureLevel::SqlServer2005),
            "EXEC ..sp_tablecollations_90 N'.[t]'"
        );
        assert_eq!(
            table_collations_sql("t", FeatureLevel::SqlServer2008),
            "EXEC ..sp_tablecollations_100 N'.[t]'"
        );
    }
}

#[cfg(test)]
mod identifier_tests {
    use super::validate_sql_identifier;

    #[test]
    fn accepts_legitimate_identifiers() {
        for ident in [
            "Foo",
            "dbo.Foo",
            "db..Foo",
            "##bulk_test",
            "#temp",
            "t$1",
            "Ünïcode",
            "*",
            "[my table]",
            "[dbo].[my table]",
            "[weird]]name]",
            "[a;b--c']",
        ] {
            assert!(validate_sql_identifier("x", ident).is_ok(), "{ident:?}");
        }
    }

    #[test]
    fn rejects_identifiers_that_would_splice_sql() {
        for ident in [
            "Foo\0bar",
            "Foo\nbar",
            "Foo]",
            "a]b",
            "[my] table]",
            "[unterminated",
            "t; DROP TABLE x--",
            "t UNION SELECT 1",
            "a,b",
            "[t]UNION(SELECT 1)",
            "f(1)UNION(SELECT 1)",
            "x)",
            "x(",
            "'quoted'",
            "a-b",
        ] {
            assert!(validate_sql_identifier("x", ident).is_err(), "{ident:?}");
        }
    }
}
