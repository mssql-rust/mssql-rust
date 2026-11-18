use crate::{sql_read_bytes::SqlReadBytes, tds::codec::VarLenContext, ColumnData, VarLenType};

pub(crate) async fn decode<R>(
    src: &mut R,
    ctx: &VarLenContext,
) -> crate::Result<ColumnData<'static>>
where
    R: SqlReadBytes + Unpin,
{
    use VarLenType::*;

    let ty = ctx.r#type();
    let len = ctx.len();
    let collation = ctx.collation();

    let res = match ty {
        Bitn => super::bit::decode(src).await?,
        Intn => super::int::decode(src, len).await?,
        Floatn => super::float::decode(src, len).await?,
        Guid => super::guid::decode(src).await?,
        BigChar | BigVarChar | NChar | NVarchar => {
            ColumnData::String(super::string::decode(src, ty, len, collation).await?)
        }
        Money => {
            let len = src.read_u8().await?;
            super::money::decode(src, len).await?
        }
        Datetimen => {
            let rlen = src.read_u8().await?;
            super::datetimen::decode(src, rlen, len as u8).await?
        }
        #[cfg(feature = "tds73")]
        Daten => super::date::decode(src).await?,
        #[cfg(feature = "tds73")]
        Timen => super::time::decode(src, len).await?,
        #[cfg(feature = "tds73")]
        Datetime2 => super::datetime2::decode(src, len).await?,
        #[cfg(feature = "tds73")]
        DatetimeOffsetn => super::datetimeoffsetn::decode(src, len).await?,
        BigBinary | BigVarBin => super::binary::decode(src, len).await?,
        Text => super::text::decode(src, collation).await?,
        NText => super::text::decode(src, None).await?,
        Image => super::image::decode(src).await?,
        SSVariant => super::sql_variant::decode(src).await?,
        // CLR UDT values always use the PLP format (MS-TDS 2.2.5.5.4),
        // whatever the declared max size, and are surfaced as their raw
        // serialized bytes. Ported from upstream Tiberius 7c1075c.
        Udt => ColumnData::Binary(super::plp::decode(src, 0xffff_ffff).await?.map(Into::into)),
        // Every VarLenType TypeInfo::decode can construct is handled above.
        t => unreachable!("{:?} is rejected in TypeInfo::decode", t),
    };

    Ok(res)
}

#[cfg(test)]
mod udt_tests {
    use super::decode;
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    use crate::tds::codec::{VarLenContext, VarLenType};
    use crate::ColumnData;
    use bytes::{BufMut, BytesMut};

    // A UDT value is PLP-framed even when its declared max size is small.
    #[tokio::test]
    async fn udt_values_are_plp_binary() {
        let ctx = VarLenContext::new(VarLenType::Udt, 892, None);

        let mut buf = BytesMut::new();
        buf.put_u64_le(0xffff_ffff_ffff_fffe);
        buf.put_u32_le(4);
        buf.put_slice(&[0xde, 0xad, 0xbe, 0xef]);
        buf.put_u32_le(0);
        let v = decode(&mut buf.into_sql_read_bytes(), &ctx).await.unwrap();
        assert_eq!(
            v,
            ColumnData::Binary(Some((&[0xde, 0xad, 0xbe, 0xef][..]).into()))
        );

        let mut buf = BytesMut::new();
        buf.put_u64_le(0xffff_ffff_ffff_ffff);
        let v = decode(&mut buf.into_sql_read_bytes(), &ctx).await.unwrap();
        assert_eq!(v, ColumnData::Binary(None));
    }
}
