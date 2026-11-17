use crate::SqlReadBytes;

#[allow(dead_code)] // we might want to debug the values
#[derive(Debug)]
pub struct TokenOrder {
    pub(crate) column_indexes: Vec<u16>,
}

impl TokenOrder {
    pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        // ORDER is a list of u16 column numbers; an odd byte length used to
        // be silently rounded down, leaving a stray byte that desynced the
        // stream (upstream Tiberius bf96855).
        let byte_len = src.read_u16_le().await?;
        if byte_len % 2 != 0 {
            return Err(crate::Error::Protocol(
                format!("ORDER: odd token length {byte_len}").into(),
            ));
        }
        let len = byte_len / 2;

        let mut column_indexes = Vec::with_capacity(len as usize);

        for _ in 0..len {
            column_indexes.push(src.read_u16_le().await?);
        }

        Ok(TokenOrder { column_indexes })
    }
}
