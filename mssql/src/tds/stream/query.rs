use crate::tds::codec::TokenReturnValue;
use crate::tds::stream::{ReceivedToken, TokenStream};
use crate::{row::ColumnType, Client, Column, Error, FromSql, Row, ToSql};
use futures_util::{
    io::{AsyncRead, AsyncWrite},
    ready,
    stream::{BoxStream, Peekable, Stream, StreamExt, TryStreamExt},
};
use std::{
    borrow::Cow,
    fmt::Debug,
    future::Future,
    ops::DerefMut,
    pin::Pin,
    sync::Arc,
    task::{self, Poll},
};

type OwnedRead<C> =
    Pin<Box<dyn Future<Output = (C, crate::Result<Option<ReceivedToken>>)> + Send + 'static>>;

/// A `'static` row stream that owns its client (or pool checkout).
///
/// Unlike [`QueryStream`], this stream is not tied to `&mut Client`. That
/// makes it suitable for repository methods which need to return a stream.
/// The owner may be any pool object implementing
/// [`DerefMut<Target = Client<_>>`](DerefMut). Dropping the stream drops its
/// owner, so pool checkouts return to their pool normally.
pub struct OwnedQueryStream<C, S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    owner: Option<C>,
    client_mut: fn(&mut C) -> &mut Client<S>,
    pending: Option<OwnedRead<C>>,
    columns: Option<Arc<Vec<Column>>>,
    result_set_index: Option<usize>,
    done: bool,
}

impl<C, S> Debug for OwnedQueryStream<C, S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedQueryStream")
            .field("has_owner", &self.owner.is_some())
            .field("done", &self.done)
            .finish()
    }
}

impl<C, S> OwnedQueryStream<C, S>
where
    C: DerefMut<Target = Client<S>> + Send + Unpin + 'static,
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    /// Start a parameterized query while taking ownership of a pool checkout.
    pub async fn query<'a>(
        mut owner: C,
        query: impl Into<Cow<'a, str>>,
        params: &'a [&'a dyn ToSql],
    ) -> crate::Result<Self> {
        owner.start_query(query, params).await?;
        Ok(Self::new(owner, |owner| owner.deref_mut()))
    }

    /// Start a SQL batch while taking ownership of a pool checkout.
    pub async fn simple_query<'a>(
        mut owner: C,
        query: impl Into<Cow<'a, str>>,
    ) -> crate::Result<Self> {
        owner.start_simple_query(query).await?;
        Ok(Self::new(owner, |owner| owner.deref_mut()))
    }
}

impl<S> OwnedQueryStream<Client<S>, S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub(crate) fn from_client(owner: Client<S>) -> Self {
        Self::new(owner, |owner| owner)
    }
}

impl<C, S> OwnedQueryStream<C, S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    fn new(owner: C, client_mut: fn(&mut C) -> &mut Client<S>) -> Self {
        Self {
            owner: Some(owner),
            client_mut,
            pending: None,
            columns: None,
            result_set_index: None,
            done: false,
        }
    }
}

impl<C, S> Stream for OwnedQueryStream<C, S>
where
    C: Send + Unpin + 'static,
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Item = crate::Result<Row>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            if this.done {
                return Poll::Ready(None);
            }

            if this.pending.is_none() {
                let mut owner = this.owner.take().expect("owner is present between reads");
                let client_mut = this.client_mut;
                this.pending = Some(Box::pin(async move {
                    let result = {
                        let client = client_mut(&mut owner);
                        let mut tokens = TokenStream::new(&mut client.connection).try_unfold();
                        tokens.try_next().await
                    };
                    (owner, result)
                }));
            }

            let (owner, token) = ready!(this.pending.as_mut().unwrap().as_mut().poll(cx));
            this.pending = None;
            this.owner = Some(owner);

            let token = match token {
                Ok(Some(token)) => token,
                Ok(None) => {
                    this.done = true;
                    return Poll::Ready(None);
                }
                Err(error) => {
                    this.done = true;
                    return Poll::Ready(Some(Err(error)));
                }
            };

            match token {
                ReceivedToken::NewResultset(meta) => {
                    this.columns = Some(Arc::new(
                        meta.columns
                            .iter()
                            .map(|column| Column {
                                name: column.col_name.to_string(),
                                column_type: ColumnType::from(&column.base.ty),
                            })
                            .collect(),
                    ));
                    this.result_set_index = Some(match this.result_set_index {
                        Some(index) => index + 1,
                        None => 0,
                    });
                }
                ReceivedToken::Row(data) => {
                    let Some(columns) = this.columns.clone() else {
                        this.done = true;
                        return Poll::Ready(Some(Err(Error::Protocol(
                            "ROW token arrived before any column metadata".into(),
                        ))));
                    };
                    return Poll::Ready(Some(Ok(Row {
                        columns,
                        data,
                        result_index: this.result_set_index.unwrap_or(0),
                    })));
                }
                ReceivedToken::Error(error) => {
                    this.done = true;
                    return Poll::Ready(Some(Err(Error::Server(error))));
                }
                _ => {}
            }
        }
    }
}

/// A set of `Streams` of [`QueryItem`] values, which can be either result
/// metadata or a row.
///
/// The `QueryStream` needs to be polled empty before sending another query to
/// the [`Client`](crate::Client), failing to do so causes a flush before the
/// next query, slowing it down in an undeterministic way.
///
/// Every stream starts with metadata, describing the structure of the incoming
/// rows, e.g. the columns in the order they are presented in every row.
///
/// If after consuming rows from the stream, another metadata result arrives, it
/// means the stream has multiple results from different queries. This new
/// metadata item will describe the next rows from here forwards.
///
/// If having one set of results in the response, using
/// [`into_row_stream`](QueryStream::into_row_stream) might be more
/// convenient to use.
///
/// The struct provides non-streaming APIs with
/// [`into_results`](QueryStream::into_results),
/// [`into_first_result`](QueryStream::into_first_result) and
/// [`into_row`](QueryStream::into_row).
///
/// # Example
///
/// ```
/// # use mssql::{Config, QueryItem};
/// # use tokio_util::compat::TokioAsyncWriteCompatExt;
/// # use std::env;
/// # use futures_util::stream::TryStreamExt;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let c_str = env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or(
/// #     "server=tcp:localhost,1433;integratedSecurity=true;TrustServerCertificate=true".to_owned(),
/// # );
/// # let config = Config::from_ado_string(&c_str)?;
/// # let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
/// # tcp.set_nodelay(true)?;
/// # let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;
/// let mut stream = client
///     .query(
///         "SELECT @P1 AS first; SELECT @P2 AS second",
///         &[&1i32, &2i32],
///     )
///     .await?;
///
/// // The stream consists of four items, in the following order:
/// // - Metadata from `SELECT 1`
/// // - The only resulting row from `SELECT 1`
/// // - Metadata from `SELECT 2`
/// // - The only resulting row from `SELECT 2`
/// while let Some(item) = stream.try_next().await? {
///     match item {
///         // our first item is the column data always
///         QueryItem::Metadata(meta) if meta.result_index() == 0 => {
///             // the first result column info can be handled here
///         }
///         // ... and from there on from 0..N rows
///         QueryItem::Row(row) if row.result_index() == 0 => {
///             assert_eq!(Some(1), row.get(0));
///         }
///         // the second result set returns first another metadata item
///         QueryItem::Metadata(meta) => {
///             // .. handling
///         }
///         // ...and, again, we get rows from the second resultset
///         QueryItem::Row(row) => {
///             assert_eq!(Some(2), row.get(0));
///         }
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub struct QueryStream<'a> {
    token_stream: Peekable<BoxStream<'a, crate::Result<ReceivedToken>>>,
    columns: Option<Arc<Vec<Column>>>,
    result_set_index: Option<usize>,
    /// `RETURNVALUE` tokens (stored-procedure OUTPUT parameters) seen so
    /// far - noted as they're read (including ones skipped over by
    /// [`forward_to_metadata`](Self::forward_to_metadata), which otherwise
    /// discards everything that isn't `NewResultset`), surfaced via
    /// [`into_output_params`](Self::into_output_params).
    return_values: Vec<TokenReturnValue>,
    /// A `RETURNSTATUS` token (a stored procedure's own `RETURN` value, if
    /// it used one), noted the same way.
    return_status: Option<i32>,
}

impl<'a> Debug for QueryStream<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryStream")
            .field(
                "token_stream",
                &"BoxStream<'a, crate::Result<ReceivedToken>>",
            )
            .finish()
    }
}

impl<'a> QueryStream<'a> {
    pub(crate) fn new(token_stream: BoxStream<'a, crate::Result<ReceivedToken>>) -> Self {
        Self {
            token_stream: token_stream.peekable(),
            columns: None,
            result_set_index: None,
            return_values: Vec::new(),
            return_status: None,
        }
    }

    /// Note a token that [`forward_to_metadata`](Self::forward_to_metadata)
    /// or [`poll_next`](Stream::poll_next) is about to discard, if it's one
    /// [`into_output_params`](Self::into_output_params) surfaces later.
    fn note_return_token(&mut self, token: ReceivedToken) {
        match token {
            ReceivedToken::ReturnValue(rv) => self.return_values.push(rv),
            ReceivedToken::ReturnStatus(status) => {
                self.return_status = Some(i32::from_le_bytes(status.to_le_bytes()))
            }
            _ => (),
        }
    }

    /// Moves the stream forward until having result metadata, stream end or an
    /// error.
    pub(crate) async fn forward_to_metadata(&mut self) -> crate::Result<()> {
        loop {
            let item = Pin::new(&mut self.token_stream)
                .peek()
                .await
                .map(|r| r.as_ref().map_err(|e| e.clone()))
                .transpose()?;

            match item {
                Some(ReceivedToken::NewResultset(_)) => break,
                Some(_) => {
                    if let Some(token) = self.token_stream.try_next().await? {
                        self.note_return_token(token);
                    }
                }
                None => break,
            }
        }

        Ok(())
    }

    /// Consumes the rest of the stream (discarding any result-set rows
    /// still in it - read those first with the `Stream`/[`into_results`](Self::into_results)
    /// APIs if you need both) and returns the stored-procedure OUTPUT
    /// parameters and `RETURN` value collected along the way. See
    /// [`Client::call_procedure`](crate::Client::call_procedure).
    pub async fn into_output_params(mut self) -> crate::Result<OutputParams> {
        while self.try_next().await?.is_some() {}

        Ok(OutputParams {
            values: self.return_values,
            status: self.return_status,
        })
    }

    /// The list of columns either for the current result set, or for the next
    /// one. If the stream is just created, or if the next item in the stream
    /// contains metadata, the metadata will be taken from the stream. Otherwise
    /// the columns will be returned from the cache and reflect on the current
    /// result set.
    ///
    /// # Example
    ///
    /// ```
    /// # use mssql::Config;
    /// # use tokio_util::compat::TokioAsyncWriteCompatExt;
    /// # use std::env;
    /// # use futures_util::stream::TryStreamExt;
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// # let c_str = env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or(
    /// #     "server=tcp:localhost,1433;integratedSecurity=true;TrustServerCertificate=true".to_owned(),
    /// # );
    /// # let config = Config::from_ado_string(&c_str)?;
    /// # let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    /// # tcp.set_nodelay(true)?;
    /// # let mut client = mssql::Client::connect(config, tcp.compat_write()).await?;
    /// let mut stream = client
    ///     .query(
    ///         "SELECT @P1 AS first; SELECT @P2 AS second",
    ///         &[&1i32, &2i32],
    ///     )
    ///     .await?;
    ///
    /// // Nothing is fetched, the first result set starts.
    /// let cols = stream.columns().await?.unwrap();
    /// assert_eq!("first", cols[0].name());
    ///
    /// // Move over the metadata.
    /// stream.try_next().await?;
    ///
    /// // We're in the first row, seeing the metadata for that set.
    /// let cols = stream.columns().await?.unwrap();
    /// assert_eq!("first", cols[0].name());
    ///
    /// // Move over the only row in the first set.
    /// stream.try_next().await?;
    ///
    /// // End of the first set, getting the metadata by peaking the next item.
    /// let cols = stream.columns().await?.unwrap();
    /// assert_eq!("second", cols[0].name());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn columns(&mut self) -> crate::Result<Option<&[Column]>> {
        use ReceivedToken::*;

        loop {
            let item = Pin::new(&mut self.token_stream)
                .peek()
                .await
                .map(|r| r.as_ref().map_err(|e| e.clone()))
                .transpose()?;

            match item {
                Some(token) => match token {
                    NewResultset(metadata) => {
                        self.columns = Some(Arc::new(metadata.columns().collect()));
                        break;
                    }
                    Row(_) => {
                        break;
                    }
                    _ => {
                        self.token_stream.try_next().await?;
                        continue;
                    }
                },
                None => {
                    break;
                }
            }
        }

        Ok(self.columns.as_ref().map(|c| c.as_slice()))
    }

    /// Collects results from all queries in the stream into memory in the order
    /// of querying.
    pub async fn into_results(self) -> crate::Result<Vec<Vec<Row>>> {
        collect_results(self).await
    }

    /// Collects the output of the first query, dropping any further
    /// results.
    pub async fn into_first_result(self) -> crate::Result<Vec<Row>> {
        let mut results = self.into_results().await?.into_iter();
        let rows = results.next().unwrap_or_default();

        Ok(rows)
    }

    /// Collects the first row from the output of the first query, dropping any
    /// further rows.
    pub async fn into_row(self) -> crate::Result<Option<Row>> {
        let mut results = self.into_first_result().await?.into_iter();

        Ok(results.next())
    }

    /// Convert the stream into a stream of rows, skipping metadata items.
    pub fn into_row_stream(self) -> BoxStream<'a, crate::Result<Row>> {
        let s = self.try_filter_map(|item| async {
            match item {
                QueryItem::Row(row) => Ok(Some(row)),
                QueryItem::Metadata(_) => Ok(None),
            }
        });

        Box::pin(s)
    }
}

/// Info about the following stream of rows.
#[derive(Debug, Clone)]
pub struct ResultMetadata {
    columns: Arc<Vec<Column>>,
    result_index: usize,
}

impl ResultMetadata {
    /// Column info. The order is the same as in the following rows.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// The number of the result set, an incrementing value starting from zero,
    /// which gives an indication of the position of the result set in the
    /// stream.
    pub fn result_index(&self) -> usize {
        self.result_index
    }
}

/// Resulting data from a query.
#[derive(Debug)]
pub enum QueryItem {
    /// A single row of data.
    Row(Row),
    /// Information of the upcoming row data.
    Metadata(ResultMetadata),
}

impl QueryItem {
    pub(crate) fn metadata(columns: Arc<Vec<Column>>, result_index: usize) -> Self {
        Self::Metadata(ResultMetadata {
            columns,
            result_index,
        })
    }

    /// Returns a reference to the metadata, if the item is of a correct variant.
    pub fn as_metadata(&self) -> Option<&ResultMetadata> {
        match self {
            QueryItem::Row(_) => None,
            QueryItem::Metadata(ref metadata) => Some(metadata),
        }
    }

    /// Returns a reference to the row, if the item is of a correct variant.
    pub fn as_row(&self) -> Option<&Row> {
        match self {
            QueryItem::Row(ref row) => Some(row),
            QueryItem::Metadata(_) => None,
        }
    }

    /// Returns the metadata, if the item is of a correct variant.
    pub fn into_metadata(self) -> Option<ResultMetadata> {
        match self {
            QueryItem::Row(_) => None,
            QueryItem::Metadata(metadata) => Some(metadata),
        }
    }

    /// Returns the row, if the item is of a correct variant.
    pub fn into_row(self) -> Option<Row> {
        match self {
            QueryItem::Row(row) => Some(row),
            QueryItem::Metadata(_) => None,
        }
    }
}

impl<'a> Stream for QueryStream<'a> {
    type Item = crate::Result<QueryItem>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            let token = match ready!(this.token_stream.poll_next_unpin(cx)) {
                Some(res) => res?,
                None => return Poll::Ready(None),
            };

            return match token {
                ReceivedToken::NewResultset(meta) => {
                    let column_meta = meta
                        .columns
                        .iter()
                        .map(|x| Column {
                            name: x.col_name.to_string(),
                            column_type: ColumnType::from(&x.base.ty),
                        })
                        .collect::<Vec<_>>();

                    let column_meta = Arc::new(column_meta);
                    this.columns = Some(column_meta.clone());

                    this.result_set_index = this.result_set_index.map(|i| i + 1);

                    let query_item =
                        QueryItem::metadata(column_meta, *this.result_set_index.get_or_insert(0));

                    return Poll::Ready(Some(Ok(query_item)));
                }
                ReceivedToken::Row(data) => {
                    // A ROW before any COLMETADATA in this stream is a protocol
                    // violation; it used to `unwrap()`-panic. (Context's
                    // `last_meta` survives across queries, so such a ROW still
                    // decodes fine before reaching here.) Ported from upstream
                    // Tiberius 3e6afc3.
                    let Some(columns) = this.columns.as_ref() else {
                        return Poll::Ready(Some(Err(crate::Error::Protocol(
                            "ROW token arrived before any column metadata".into(),
                        ))));
                    };
                    let columns = columns.clone();
                    let result_index = this.result_set_index.unwrap_or(0);

                    let row = Row {
                        columns,
                        data,
                        result_index,
                    };

                    Poll::Ready(Some(Ok(QueryItem::Row(row))))
                }
                other => {
                    this.note_return_token(other);
                    continue;
                }
            };
        }
    }
}

/// The stored-procedure OUTPUT parameters and `RETURN` value collected from
/// a [`QueryStream`] via [`into_output_params`](QueryStream::into_output_params).
/// See [`Client::call_procedure`](crate::Client::call_procedure).
#[derive(Debug)]
pub struct OutputParams {
    values: Vec<TokenReturnValue>,
    status: Option<i32>,
}

impl OutputParams {
    /// The stored procedure's own `RETURN` value, if it used one - `None`
    /// if the procedure has no `RETURN` statement (or wasn't actually
    /// called via [`Client::call_procedure`](crate::Client::call_procedure),
    /// e.g. this came from a plain query).
    pub fn return_status(&self) -> Option<i32> {
        self.status
    }

    /// The value of the named OUTPUT parameter, converted via [`FromSql`].
    /// The name may be given with or without its leading `@`, matching
    /// whichever way it was declared with in
    /// [`ProcParam::output`](crate::ProcParam::output).
    ///
    /// # Panics
    ///
    /// - No OUTPUT parameter with this name was bound.
    /// - The requested type conversion (SQL -> Rust) is not possible.
    ///
    /// Use [`try_get`](Self::try_get) for a non-panicking version.
    #[track_caller]
    pub fn get<'a, T: FromSql<'a>>(&'a self, name: &str) -> Option<T> {
        self.try_get(name).unwrap()
    }

    /// Retrieve the value of the named OUTPUT parameter, converted via
    /// [`FromSql`]. See [`get`](Self::get).
    pub fn try_get<'a, T: FromSql<'a>>(&'a self, name: &str) -> crate::Result<Option<T>> {
        let name = name.trim_start_matches('@');

        let found = self
            .values
            .iter()
            .find(|rv| rv.param_name.trim_start_matches('@') == name)
            .ok_or_else(|| Error::Conversion(format!("no such output parameter: {name}").into()))?;

        T::from_sql(&found.value)
    }
}

/// Groups a stream of query items into result sets: each `Metadata` item
/// starts a new set, and rows go into the current one. An empty result set
/// (metadata with no rows) is kept, which is the #385 fix (d346db2). A row
/// with no preceding metadata isn't dropped, though `QueryStream` now
/// rejects that case outright (68345bd). Ported from upstream Tiberius
/// (tiberius-rs/tiberius 38896ba), which made this a standalone function so
/// it can be unit-tested over a plain stream.
async fn collect_results<S>(mut stream: S) -> crate::Result<Vec<Vec<Row>>>
where
    S: Stream<Item = crate::Result<QueryItem>> + Unpin,
{
    let mut results: Vec<Vec<Row>> = Vec::new();
    let mut current: Option<Vec<Row>> = None;

    while let Some(item) = stream.try_next().await? {
        match item {
            QueryItem::Metadata(_) => {
                // A new result set begins. Flush the previous one (if any) and
                // open a fresh, empty set so a zero-row result still produces an
                // inner Vec at the correct position.
                if let Some(previous) = current.take() {
                    results.push(previous);
                }
                current = Some(Vec::new());
            }
            QueryItem::Row(row) => {
                // Rows are always preceded by their metadata in a well-formed
                // stream, so `current` is normally `Some`. Be defensive and open
                // a result set on the fly rather than discard a leading row.
                current.get_or_insert_with(Vec::new).push(row);
            }
        }
    }

    if let Some(last) = current.take() {
        results.push(last);
    }

    Ok(results)
}

#[cfg(test)]
mod collect_results_tests {
    use super::*;
    use crate::row::ColumnType;
    use crate::tds::codec::{
        BaseMetaDataColumn, FixedLenType, MetaDataColumn, TokenColMetaData, TokenRow, TypeInfo,
    };
    use crate::Column;
    use enumflags2::BitFlags;
    use futures_util::stream;
    use std::future;

    fn columns() -> Arc<Vec<Column>> {
        Arc::new(vec![Column::new("c".to_string(), ColumnType::Int4)])
    }

    fn meta(cols: &Arc<Vec<Column>>, result_index: usize) -> QueryItem {
        QueryItem::metadata(cols.clone(), result_index)
    }

    fn row(cols: &Arc<Vec<Column>>, result_index: usize) -> QueryItem {
        QueryItem::Row(Row {
            columns: cols.clone(),
            data: TokenRow::new(),
            result_index,
        })
    }

    async fn collect(items: Vec<QueryItem>) -> Vec<Vec<Row>> {
        let stream = stream::iter(items.into_iter().map(Ok::<_, crate::error::Error>));
        collect_results(stream).await.expect("collect_results")
    }

    #[tokio::test]
    async fn empty_stream_yields_no_result_sets() {
        assert!(collect(vec![]).await.is_empty());
    }

    #[tokio::test]
    async fn metadata_without_rows_yields_one_empty_result_set() {
        let cols = columns();
        let results = collect(vec![meta(&cols, 0)]).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].is_empty());
    }

    #[tokio::test]
    async fn multiple_empty_result_sets_are_preserved() {
        let cols = columns();
        let results = collect(vec![meta(&cols, 0), meta(&cols, 1)]).await;
        assert_eq!(results.len(), 2);
        assert!(results[0].is_empty());
        assert!(results[1].is_empty());
    }

    #[tokio::test]
    async fn rows_are_grouped_by_result_set_in_order() {
        let cols = columns();
        let results = collect(vec![
            meta(&cols, 0),
            row(&cols, 0),
            row(&cols, 0),
            meta(&cols, 1),
            row(&cols, 1),
        ])
        .await;

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].len(), 2);
        assert_eq!(results[1].len(), 1);
    }

    #[tokio::test]
    async fn leading_row_is_not_discarded() {
        // The old implementation consumed and threw away the first stream item,
        // assuming it was metadata. If a row led, it was silently lost. The
        // robust implementation keeps every row: a stream of two leading rows
        // must produce one result set containing BOTH rows (the old logic would
        // have produced a single-row set).
        let cols = columns();
        let results = collect(vec![row(&cols, 0), row(&cols, 0)]).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].len(), 2);
    }

    fn wire_metadata() -> Arc<TokenColMetaData<'static>> {
        Arc::new(TokenColMetaData {
            columns: vec![MetaDataColumn {
                base: BaseMetaDataColumn {
                    flags: BitFlags::empty(),
                    ty: TypeInfo::FixedLen(FixedLenType::Int4),
                },
                col_name: Cow::Borrowed("answer"),
            }],
        })
    }

    fn query_stream(tokens: Vec<crate::Result<ReceivedToken>>) -> QueryStream<'static> {
        QueryStream::new(Box::pin(stream::iter(tokens)))
    }

    #[tokio::test]
    async fn query_stream_covers_metadata_rows_and_convenience_collectors() {
        let metadata = wire_metadata();
        let mut data = TokenRow::new();
        data.push(crate::ColumnData::I32(Some(42)));
        let tokens = vec![
            Ok(ReceivedToken::NewResultset(metadata)),
            Ok(ReceivedToken::Row(data)),
        ];

        let mut query = query_stream(tokens);
        assert!(format!("{query:?}").contains("QueryStream"));
        assert_eq!("answer", query.columns().await.unwrap().unwrap()[0].name());

        let metadata = query.try_next().await.unwrap().unwrap();
        assert!(metadata.as_metadata().is_some());
        assert!(metadata.as_row().is_none());
        assert_eq!(0, metadata.as_metadata().unwrap().result_index());
        assert_eq!(
            "answer",
            metadata.as_metadata().unwrap().columns()[0].name()
        );

        let result_row = query.try_next().await.unwrap().unwrap();
        assert!(result_row.as_metadata().is_none());
        assert_eq!(Some(42), result_row.as_row().unwrap().get::<i32, _>(0));
        assert!(result_row.into_row().is_some());
        assert!(query.try_next().await.unwrap().is_none());

        let metadata_item = QueryItem::metadata(columns(), 3);
        assert_eq!(3, metadata_item.into_metadata().unwrap().result_index());

        let row_item = row(&columns(), 0);
        assert!(row_item.into_metadata().is_none());

        let empty = query_stream(Vec::new());
        assert!(empty.into_results().await.unwrap().is_empty());
        let empty = query_stream(Vec::new());
        assert!(empty.into_first_result().await.unwrap().is_empty());
        let empty = query_stream(Vec::new());
        assert!(empty.into_row().await.unwrap().is_none());
        let mut empty = query_stream(Vec::new());
        assert!(empty.columns().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn query_stream_reports_bad_order_and_forwards_stream_errors() {
        let mut data = TokenRow::new();
        data.push(crate::ColumnData::I32(Some(1)));
        let mut query = query_stream(vec![Ok(ReceivedToken::Row(data))]);
        assert!(matches!(
            query.try_next().await.unwrap_err(),
            Error::Protocol(_)
        ));

        let mut query = query_stream(vec![Err(Error::Protocol("sentinel".into()))]);
        assert!(matches!(
            query.columns().await.unwrap_err(),
            Error::Protocol(_)
        ));
    }

    #[tokio::test]
    async fn row_stream_filters_metadata_and_keeps_rows() {
        let metadata = wire_metadata();
        let mut data = TokenRow::new();
        data.push(crate::ColumnData::I32(Some(7)));
        let query = query_stream(vec![
            Ok(ReceivedToken::NewResultset(metadata)),
            Ok(ReceivedToken::Row(data)),
        ]);
        let rows = query
            .into_row_stream()
            .try_collect::<Vec<_>>()
            .await
            .unwrap();
        assert_eq!(1, rows.len());
        assert_eq!(Some(7), rows[0].get::<i32, _>(0));
    }

    type MemoryStream = futures_util::io::Cursor<Vec<u8>>;

    fn unused_client(_: &mut ()) -> &mut Client<MemoryStream> {
        panic!("a preloaded owned-stream future must not request its owner")
    }

    fn preloaded_owned_stream(
        token: crate::Result<Option<ReceivedToken>>,
    ) -> OwnedQueryStream<(), MemoryStream> {
        OwnedQueryStream {
            owner: None,
            client_mut: unused_client,
            pending: Some(Box::pin(future::ready(((), token)))),
            columns: None,
            result_set_index: None,
            done: false,
        }
    }

    #[tokio::test]
    async fn owned_stream_reports_protocol_server_transport_and_end_conditions() {
        let mut row = TokenRow::new();
        row.push(crate::ColumnData::I32(Some(1)));
        let mut stream = preloaded_owned_stream(Ok(Some(ReceivedToken::Row(row))));
        assert!(matches!(stream.try_next().await, Err(Error::Protocol(_))));
        assert!(stream.try_next().await.unwrap().is_none());

        let server = crate::tds::codec::TokenError {
            code: 50000,
            state: 1,
            class: 16,
            message: "sentinel".into(),
            server: "server".into(),
            procedure: "".into(),
            line: 1,
        };
        let mut stream = preloaded_owned_stream(Ok(Some(ReceivedToken::Error(server))));
        assert!(matches!(stream.try_next().await, Err(Error::Server(_))));

        let mut stream = preloaded_owned_stream(Err(Error::Protocol("sentinel".into())));
        assert!(matches!(stream.try_next().await, Err(Error::Protocol(_))));

        let mut stream = preloaded_owned_stream(Ok(None));
        assert!(stream.try_next().await.unwrap().is_none());
    }

    fn return_value(name: &str, value: i32) -> ReceivedToken {
        ReceivedToken::ReturnValue(TokenReturnValue {
            param_ordinal: 0,
            param_name: name.to_string(),
            udf: false,
            meta: BaseMetaDataColumn {
                flags: BitFlags::empty(),
                ty: TypeInfo::FixedLen(FixedLenType::Int4),
            },
            value: crate::ColumnData::I32(Some(value)),
        })
    }

    #[test]
    fn owned_stream_debug_reports_owner_and_completion() {
        let stream = preloaded_owned_stream(Ok(None));
        assert_eq!(
            format!("{stream:?}"),
            "OwnedQueryStream { has_owner: false, done: false }"
        );
    }

    #[test]
    fn query_item_accessors_distinguish_metadata_from_rows() {
        let cols = columns();
        let metadata = meta(&cols, 3);
        assert!(metadata.as_row().is_none());
        assert_eq!(metadata.as_metadata().unwrap().result_index(), 3);
        assert_eq!(metadata.as_metadata().unwrap().columns().len(), 1);
        assert!(meta(&cols, 3).into_row().is_none());
        assert!(metadata.into_metadata().is_some());

        let item = row(&cols, 1);
        assert!(item.as_metadata().is_none());
        assert!(item.as_row().is_some());
        assert!(row(&cols, 1).into_metadata().is_none());
        assert!(item.into_row().is_some());
    }

    #[tokio::test]
    async fn output_params_collect_return_values_and_status() {
        let query = query_stream(vec![
            Ok(ReceivedToken::ReturnStatus(5)),
            Ok(return_value("@total", 42)),
            Ok(ReceivedToken::ReturnStatus(u32::MAX)),
        ]);
        let params = query.into_output_params().await.unwrap();
        // The last status wins, reinterpreted as a signed value.
        assert_eq!(params.return_status(), Some(-1));
        assert_eq!(params.get::<i32>("total"), Some(42));
        assert_eq!(params.get::<i32>("@total"), Some(42));
        assert_eq!(params.try_get::<i32>("@total").unwrap(), Some(42));
        assert!(matches!(
            params.try_get::<i32>("missing"),
            Err(Error::Conversion(_))
        ));
        assert!(params.try_get::<&str>("total").is_err());
    }

    #[tokio::test]
    async fn output_params_of_a_plain_query_are_empty() {
        let params = query_stream(vec![]).into_output_params().await.unwrap();
        assert_eq!(params.return_status(), None);
        assert!(params.try_get::<i32>("anything").is_err());
    }

    #[tokio::test]
    async fn columns_skips_other_tokens_and_stops_at_rows_and_the_end() {
        // Tokens before the metadata are consumed; the metadata is not.
        let mut query = query_stream(vec![
            Ok(ReceivedToken::ReturnStatus(0)),
            Ok(ReceivedToken::NewResultset(wire_metadata())),
        ]);
        let names: Vec<_> = query
            .columns()
            .await
            .unwrap()
            .unwrap()
            .iter()
            .map(|column| column.name().to_string())
            .collect();
        assert_eq!(names, ["answer"]);

        // A row with no metadata ends the search; so does an empty stream.
        let mut data = TokenRow::new();
        data.push(crate::ColumnData::I32(Some(1)));
        let mut query = query_stream(vec![Ok(ReceivedToken::Row(data))]);
        assert!(query.columns().await.unwrap().is_none());
        let mut query = query_stream(vec![]);
        assert!(query.columns().await.unwrap().is_none());

        // A stream error is surfaced.
        let mut query = query_stream(vec![Err(Error::Protocol("boom".into()))]);
        assert!(query.columns().await.is_err());
    }

    #[tokio::test]
    async fn forward_to_metadata_notes_return_tokens_and_stops_at_a_resultset() {
        let mut query = query_stream(vec![
            Ok(ReceivedToken::ReturnStatus(7)),
            Ok(return_value("out", 1)),
            Ok(ReceivedToken::NewResultset(wire_metadata())),
        ]);
        query.forward_to_metadata().await.unwrap();
        let params = query.into_output_params().await.unwrap();
        assert_eq!(params.return_status(), Some(7));
        assert_eq!(params.get::<i32>("out"), Some(1));

        let mut query = query_stream(vec![Ok(ReceivedToken::ReturnStatus(1))]);
        query.forward_to_metadata().await.unwrap();

        let mut query = query_stream(vec![Err(Error::Protocol("boom".into()))]);
        assert!(query.forward_to_metadata().await.is_err());
    }
}
