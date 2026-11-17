use crate::SqlReadBytes;

#[allow(dead_code)] // we might want to debug the values
#[derive(Debug)]
pub struct TokenInfo {
    /// info number
    pub(crate) number: u32,
    /// error state
    pub(crate) state: u8,
    /// severity (<10: Info)
    pub(crate) class: u8,
    pub(crate) message: String,
    pub(crate) server: String,
    pub(crate) procedure: String,
    pub(crate) line: u32,
}

impl TokenInfo {
    pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let mut body = super::TokenBody::read(src, "INFO").await?;

        let number = body.u32_le()?;
        let state = body.u8()?;
        let class = body.u8()?;
        let message = body.us_varchar()?;
        let server = body.b_varchar()?;
        let procedure = body.b_varchar()?;
        let line = body.u32_le()?;

        Ok(TokenInfo {
            number,
            state,
            class,
            message,
            server,
            procedure,
            line,
        })
    }
}
