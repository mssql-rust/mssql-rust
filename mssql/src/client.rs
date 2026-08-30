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
    BulkLoadRequest, ColumnFlag, ColumnOrderHint, MetaDataColumn, SortOrder, SqlBulkCopyOption,
    SqlBulkCopyOptions, SqlReadBytes, ToSql,
};
use codec::{
    BatchRequest, ColumnData, PacketHeader, RpcParam, RpcProcId, TokenRpcRequest, TypeInfo,
    VarLenContext, VarLenType,
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
        self.connection.flush_stream().await?;
        let rpc_params = Self::rpc_params(query);

        let params = params.iter().map(|p| p.to_sql());
        self.rpc_perform_query(RpcProcId::ExecuteSQL, rpc_params, params)
            .await?;

        let ts = TokenStream::new(&mut self.connection);
        let mut result = QueryStream::new(ts.try_unfold());
        result.forward_to_metadata().await?;

        Ok(result)
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
        self.connection.flush_stream().await?;

        let req = BatchRequest::new(query, self.connection.context().transaction_descriptor());

        let id = self.connection.context_mut().next_packet_id();
        self.connection.send(PacketHeader::batch(id), req).await?;

        let ts = TokenStream::new(&mut self.connection);

        let mut result = QueryStream::new(ts.try_unfold());
        result.forward_to_metadata().await?;

        Ok(result)
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
    /// assert_eq!(3, res.total());
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
        self.connection.flush_stream().await?;

        let columns_list = columns.join(", ");
        let query = format!("SELECT TOP 0 {columns_list} FROM {table}");

        let req = BatchRequest::new(query, self.connection.context().transaction_descriptor());

        let id = self.connection.context_mut().next_packet_id();
        self.connection.send(PacketHeader::batch(id), req).await?;

        let token_stream = TokenStream::new(&mut self.connection).try_unfold();

        let columns = token_stream
            .try_fold(None, |mut columns, token| async move {
                if let ReceivedToken::NewResultset(metadata) = token {
                    columns = Some(metadata.columns.clone());
                };

                Ok(columns)
            })
            .await?;

        columns.ok_or_else(|| {
            crate::Error::Protocol("expecting column metadata from query but not found".into())
        })
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
    ///     CREATE TABLE ##bulk_test (
    ///         id INT IDENTITY PRIMARY KEY,
    ///         foo INT NOT NULL,
    ///         bar FLOAT NOT NULL
    ///     )
    /// "#;
    ///
    /// client.simple_query(create_table).await?;
    ///
    /// // Start the bulk insert with the client.
    /// let mut req = client.bulk_insert_columns("##bulk_test", &["foo", "bar"]).await?;
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
    /// assert_eq!(3, res.total());
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
    /// assert_eq!(3, res.total());
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

        let columns: Vec<_> = self
            .column_metadata(table, columns)
            .await?
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
        let col_data = columns.iter().map(|c| format!("{}", c)).join(", ");
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

                    format!("[{col}] {dir}")
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

        BulkLoadRequest::new(&mut self.connection, columns)
    }

    /// Closes this database connection explicitly.
    pub async fn close(self) -> crate::Result<()> {
        self.connection.close().await
    }

    pub(crate) fn rpc_params<'a>(query: impl Into<Cow<'a, str>>) -> Vec<RpcParam<'a>> {
        vec![
            RpcParam {
                name: Cow::Borrowed("stmt"),
                flags: BitFlags::empty(),
                value: ColumnData::String(Some(query.into())),
                type_info: None,
            },
            RpcParam {
                name: Cow::Borrowed("params"),
                flags: BitFlags::empty(),
                value: ColumnData::I32(Some(0)),
                type_info: None,
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
