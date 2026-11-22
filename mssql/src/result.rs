pub use crate::tds::stream::{QueryItem, ResultMetadata};
use crate::{
    client::Connection,
    tds::stream::{ReceivedToken, TokenStream},
};
use futures_util::io::{AsyncRead, AsyncWrite};
use futures_util::stream::TryStreamExt;
use std::fmt::Debug;

/// A result from a query execution, listing the number of affected rows.
///
/// If executing multiple queries, the resulting counts will be come separately,
/// marking the rows affected for each query.
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
/// let result = client
///     .execute(
///         "INSERT INTO #Test (id) VALUES (@P1); INSERT INTO #Test (id) VALUES (@P2, @P3)",
///         &[&1i32, &2i32, &3i32],
///     )
///     .await?;
///
/// assert_eq!(&[1, 2], result.rows_affected());
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct ExecuteResult {
    rows_affected: Vec<u64>,
}

impl<'a> ExecuteResult {
    pub(crate) async fn new<S: AsyncRead + AsyncWrite + Unpin + Send>(
        connection: &'a mut Connection<S>,
    ) -> crate::Result<Self> {
        let mut token_stream = TokenStream::new(connection).try_unfold();
        let mut rows_affected = Vec::new();

        while let Some(token) = token_stream.try_next().await? {
            match token {
                ReceivedToken::DoneProc(done) if done.is_final() => (),
                ReceivedToken::DoneProc(done) => rows_affected.push(done.rows()),
                ReceivedToken::DoneInProc(done) => rows_affected.push(done.rows()),
                ReceivedToken::Done(done) => rows_affected.push(done.rows()),
                _ => (),
            }
        }

        Ok(Self { rows_affected })
    }

    /// A slice of numbers of rows affected in the same order as the given
    /// queries.
    pub fn rows_affected(&self) -> &[u64] {
        self.rows_affected.as_slice()
    }

    /// Aggregates all resulting row counts into a sum.
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
    /// let rows_affected = client
    ///     .execute(
    ///         "INSERT INTO #Test (id) VALUES (@P1); INSERT INTO #Test (id) VALUES (@P2, @P3)",
    ///         &[&1i32, &2i32, &3i32],
    ///     )
    ///     .await?;
    ///
    /// assert_eq!(3, rows_affected.total()?);
    /// # Ok(())
    /// # }
    pub fn total(self) -> crate::Result<u64> {
        // A total past `u64::MAX` rows is not representable; it is an error,
        // not a bound (RCA-IN-6).
        self.checked_total()
            .ok_or_else(|| crate::num::overflow("rows_affected_total"))
    }

    /// Like [`total`](Self::total), but `None` rather than an error when the
    /// total number of affected rows does not fit a `u64`.
    pub fn checked_total(&self) -> Option<u64> {
        self.rows_affected
            .iter()
            .try_fold(0_u64, |total, rows| total.checked_add(*rows))
    }
}

impl IntoIterator for ExecuteResult {
    type Item = u64;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows_affected.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(rows: &[u64]) -> ExecuteResult {
        ExecuteResult {
            rows_affected: rows.to_vec(),
        }
    }

    #[test]
    fn total_sums_every_statement() {
        assert_eq!(result(&[]).total().unwrap(), 0);
        assert_eq!(result(&[1, 2, 3]).total().unwrap(), 6);
        assert_eq!(result(&[u64::MAX]).total().unwrap(), u64::MAX);
    }

    #[test]
    fn checked_total_is_none_on_overflow() {
        assert_eq!(result(&[u64::MAX, 0]).checked_total(), Some(u64::MAX));
        assert_eq!(result(&[u64::MAX, 1]).checked_total(), None);
    }

    #[test]
    fn total_rejects_an_overflowing_sum() {
        for rows in [&[u64::MAX, 1][..], &[u64::MAX, u64::MAX][..]] {
            assert_eq!(
                result(rows).total().unwrap_err(),
                crate::Error::Conversion("rows_affected_total: arithmetic overflow".into())
            );
        }
    }
}
