use std::borrow::Cow;

use byteorder::{ByteOrder, LittleEndian};

use crate::{error::Error, sql_read_bytes::SqlReadBytes, tds::Collation, VarLenType};

pub(crate) async fn decode<R>(
    src: &mut R,
    ty: VarLenType,
    len: usize,
    collation: Option<Collation>,
) -> crate::Result<Option<Cow<'static, str>>>
where
    R: SqlReadBytes + Unpin,
{
    use VarLenType::*;

    let data = super::plp::decode(src, len).await?;

    match (data, ty) {
        // Codepages other than UTF
        (Some(buf), BigChar) | (Some(buf), BigVarChar) => {
            // TypeInfo::decode always reads a collation for these types, so
            // this is unreachable from server bytes; error rather than panic.
            let collation = collation
                .as_ref()
                .ok_or_else(|| Error::Protocol("char/varchar column without a collation".into()))?;
            let lossy = src.context().lossy_codepage_decoding();
            let s = collation.codec()?.decode_with(buf.as_ref(), lossy)?;

            Ok(Some(s.into()))
        }
        // UTF-16
        (Some(buf), _) => {
            if buf.len() % 2 != 0 {
                return Err(Error::Protocol("nvarchar: invalid plp length".into()));
            }

            let buf: Vec<_> = buf.chunks(2).map(LittleEndian::read_u16).collect();
            let value = match ty {
                NChar | NVarchar => String::from_utf16_lossy(&buf),
                _ => String::from_utf16(&buf)?,
            };
            Ok(Some(value.into()))
        }
        _ => Ok(None),
    }
}
