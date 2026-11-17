use crate::{tds::codec::FeatureLevel, SqlReadBytes};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
/// An error token returned from the server.
pub struct TokenError {
    /// ErrorCode
    pub(crate) code: u32,
    /// ErrorState (describing code)
    pub(crate) state: u8,
    /// The class (severity) of the error
    pub(crate) class: u8,
    /// The error message
    pub(crate) message: String,
    pub(crate) server: String,
    pub(crate) procedure: String,
    pub(crate) line: u32,
}

impl TokenError {
    pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let wide_line = src.context().version() > FeatureLevel::SqlServer2005;
        let mut body = super::TokenBody::read(src, "ERROR").await?;

        let code = body.u32_le()?;
        let state = body.u8()?;
        let class = body.u8()?;

        let message = body.us_varchar()?;
        let server = body.b_varchar()?;
        let procedure = body.b_varchar()?;

        let line = if wide_line {
            body.u32_le()?
        } else {
            body.u16_le()? as u32
        };

        let token = TokenError {
            code,
            state,
            class,
            message,
            server,
            procedure,
            line,
        };

        Ok(token)
    }

    /// The error code, see descriptions from [the manual].
    ///
    /// [the manual]: https://docs.microsoft.com/en-us/sql/relational-databases/errors-events/database-engine-events-and-errors?view=sql-server-ver15
    pub fn code(&self) -> u32 {
        self.code
    }

    /// The error state, used as a modifier to the error number.
    pub fn state(&self) -> u8 {
        self.state
    }

    /// The class (severity) of the error. A class of less than 10 indicates an
    /// informational message.
    pub fn class(&self) -> u8 {
        self.class
    }

    /// The error message returned from the server.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The server name.
    pub fn server(&self) -> &str {
        &self.server
    }

    /// The name of the stored procedure causing the error.
    pub fn procedure(&self) -> &str {
        &self.procedure
    }

    /// The line number in the SQL batch or stored procedure that caused the
    /// error. Line numbers begin at 1. If the line number is not applicable to
    /// the message, the value is 0.
    pub fn line(&self) -> u32 {
        self.line
    }
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "'{}' on server {} executing {} on line {} (code: {}, state: {}, class: {})",
            self.message, self.server, self.procedure, self.line, self.code, self.state, self.class
        )
    }
}
