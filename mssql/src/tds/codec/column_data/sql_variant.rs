//! Decoding of `sql_variant` (0x62) values, MS-TDS 2.2.5.5.3:
//!
//! ```text
//! totalLen  u32         length of everything that follows; 0 means NULL
//! baseType  u8          the TYPE byte of the stored value
//! propBytes u8          count of type-specific property bytes
//! propData  propBytes   e.g. collation, precision/scale, time scale
//! value     totalLen - 2 - propBytes bytes
//! ```
//!
//! The value is surfaced as the `ColumnData` variant of its base type (an
//! `int` variant becomes `ColumnData::I32`). A NULL variant has no base type
//! and is surfaced as `ColumnData::String(None)`.
//!
//! Ported from upstream Tiberius (tiberius-rs/tiberius ee4450b, b0cf775 and
//! 3edcc56, by Matthew Jackson). Thanks to the Tiberius team. Fork
//! additions: fixed-length base types are checked against their exact
//! size, and the numeric path validates precision/scale and uses checked
//! magnitude arithmetic.

use std::convert::TryFrom;

use crate::{
    error::Error,
    sql_read_bytes::SqlReadBytes,
    tds::{Collation, Numeric},
    ColumnData, FixedLenType, VarLenType,
};

/// `sql_variant` can't hold a `(max)`/LOB value, so no payload exceeds this.
const MAX_VARIANT_PAYLOAD: usize = 8000;

fn protocol(msg: String) -> Error {
    Error::Protocol(format!("sql_variant: {msg}").into())
}

fn expect(what: &str, got: usize, want: usize) -> crate::Result<()> {
    if got == want {
        Ok(())
    } else {
        Err(protocol(format!("expected {want} {what}, got {got}")))
    }
}

async fn read_bytes<R>(src: &mut R, len: usize) -> crate::Result<Vec<u8>>
where
    R: SqlReadBytes + Unpin,
{
    let mut buf = Vec::new();
    crate::sql_read_bytes::read_bytes_into(src, &mut buf, len).await?;
    Ok(buf)
}

async fn read_collation<R>(src: &mut R) -> crate::Result<Collation>
where
    R: SqlReadBytes + Unpin,
{
    let info = src.read_u32_le().await?;
    let sort_id = src.read_u8().await?;
    Ok(Collation::new(info, sort_id))
}

/// The exact value size of a fixed-length base type.
fn fixed_size(ty: FixedLenType) -> Option<usize> {
    Some(match ty {
        FixedLenType::Null => return None,
        FixedLenType::Bit | FixedLenType::Int1 => 1,
        FixedLenType::Int2 => 2,
        FixedLenType::Int4 | FixedLenType::Float4 | FixedLenType::Money4 => 4,
        FixedLenType::Datetime4 => 4,
        FixedLenType::Int8 | FixedLenType::Float8 | FixedLenType::Money => 8,
        FixedLenType::Datetime => 8,
    })
}

pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<ColumnData<'static>>
where
    R: SqlReadBytes + Unpin,
{
    let total_len = src.read_u32_le().await? as usize;

    if total_len == 0 {
        return Ok(ColumnData::String(None));
    }

    if total_len < 2 {
        return Err(protocol(format!("invalid total length {total_len}")));
    }

    let base_type = src.read_u8().await?;
    let prop_bytes = src.read_u8().await? as usize;

    let data_len = total_len.checked_sub(2 + prop_bytes).ok_or_else(|| {
        protocol(format!(
            "total length {total_len} < {prop_bytes} property bytes"
        ))
    })?;

    if data_len > MAX_VARIANT_PAYLOAD {
        return Err(protocol(format!(
            "value length {data_len} exceeds the maximum"
        )));
    }

    if let Ok(fixed) = FixedLenType::try_from(base_type) {
        expect("property bytes", prop_bytes, 0)?;
        let size =
            fixed_size(fixed).ok_or_else(|| protocol(format!("invalid base type {fixed:?}")))?;
        // Without this check a wrong length leaves bytes behind (or reads
        // into the next column) and desyncs the rest of the row.
        expect("value bytes", data_len, size)?;
        return super::fixed_len::decode(src, &fixed).await;
    }

    let var = VarLenType::try_from(base_type)
        .map_err(|_| protocol(format!("unknown base type 0x{base_type:02x}")))?;

    let value = match var {
        VarLenType::Guid => {
            expect("property bytes", prop_bytes, 0)?;
            expect("value bytes", data_len, 16)?;
            // No length byte here, unlike a guid column.
            let mut data = [0u8; 16];
            data.copy_from_slice(&read_bytes(src, 16).await?);
            crate::tds::codec::guid::reorder_bytes(&mut data);
            ColumnData::Guid(Some(uuid::Uuid::from_bytes(data)))
        }
        VarLenType::Decimaln | VarLenType::Numericn => {
            expect("property bytes", prop_bytes, 2)?;
            let precision = src.read_u8().await?;
            let scale = src.read_u8().await?;
            if !(1..=38).contains(&precision) || scale > precision {
                return Err(protocol(format!(
                    "invalid precision {precision} / scale {scale}"
                )));
            }
            decode_numeric(src, data_len, scale).await?
        }
        VarLenType::BigChar | VarLenType::BigVarChar => {
            expect("property bytes", prop_bytes, 7)?;
            let collation = read_collation(src).await?;
            let _max_len = src.read_u16_le().await?;
            let buf = read_bytes(src, data_len).await?;
            let lossy = src.context().lossy_codepage_decoding();
            ColumnData::String(Some(collation.codec()?.decode_with(&buf, lossy)?.into()))
        }
        VarLenType::NChar | VarLenType::NVarchar => {
            expect("property bytes", prop_bytes, 7)?;
            let _collation = read_collation(src).await?;
            let _max_len = src.read_u16_le().await?;
            if data_len % 2 != 0 {
                return Err(protocol(format!("odd nvarchar length {data_len}")));
            }
            let buf = read_bytes(src, data_len).await?;
            let units: Vec<u16> = buf
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect();
            // Lossy, like this crate's regular nvarchar decoding (e083aa7).
            ColumnData::String(Some(String::from_utf16_lossy(&units).into()))
        }
        VarLenType::BigBinary | VarLenType::BigVarBin => {
            expect("property bytes", prop_bytes, 2)?;
            let _max_len = src.read_u16_le().await?;
            ColumnData::Binary(Some(read_bytes(src, data_len).await?.into()))
        }
        #[cfg(feature = "tds73")]
        VarLenType::Daten => {
            expect("property bytes", prop_bytes, 0)?;
            expect("value bytes", data_len, 3)?;
            ColumnData::Date(Some(crate::tds::time::Date::decode(src).await?))
        }
        #[cfg(feature = "tds73")]
        VarLenType::Timen => {
            expect("property bytes", prop_bytes, 1)?;
            let scale = src.read_u8().await? as usize;
            ColumnData::Time(Some(
                crate::tds::time::Time::decode(src, scale, data_len).await?,
            ))
        }
        #[cfg(feature = "tds73")]
        VarLenType::Datetime2 => {
            expect("property bytes", prop_bytes, 1)?;
            let scale = src.read_u8().await? as usize;
            let time_len = data_len
                .checked_sub(3)
                .ok_or_else(|| protocol("datetime2 value too short".into()))?;
            ColumnData::DateTime2(Some(
                crate::tds::time::DateTime2::decode(src, scale, time_len).await?,
            ))
        }
        #[cfg(feature = "tds73")]
        VarLenType::DatetimeOffsetn => {
            expect("property bytes", prop_bytes, 1)?;
            let scale = src.read_u8().await? as usize;
            let time_len = data_len
                .checked_sub(5)
                .and_then(|n| u8::try_from(n).ok())
                .ok_or_else(|| protocol("invalid datetimeoffset value length".into()))?;
            ColumnData::DateTimeOffset(Some(
                crate::tds::time::DateTimeOffset::decode(src, scale, time_len).await?,
            ))
        }
        other => return Err(protocol(format!("unsupported base type {other:?}"))),
    };

    Ok(value)
}

/// A variant's numeric value: one sign byte, then a 4/8/12/16-byte
/// little-endian magnitude, with no leading length byte (unlike a column).
async fn decode_numeric<R>(
    src: &mut R,
    data_len: usize,
    scale: u8,
) -> crate::Result<ColumnData<'static>>
where
    R: SqlReadBytes + Unpin,
{
    let magnitude_len = data_len
        .checked_sub(1)
        .ok_or_else(|| protocol("empty numeric value".into()))?;
    if ![4, 8, 12, 16].contains(&magnitude_len) {
        return Err(protocol(format!(
            "invalid numeric magnitude length {magnitude_len}"
        )));
    }

    let negative = match src.read_u8().await? {
        0 => true,
        1 => false,
        _ => return Err(protocol("invalid numeric sign".into())),
    };

    let bytes = read_bytes(src, magnitude_len).await?;
    let mut le = [0u8; 16];
    le[..magnitude_len].copy_from_slice(&bytes);
    let magnitude = i128::try_from(u128::from_le_bytes(le))
        .map_err(|_| protocol("numeric magnitude out of range".into()))?;

    let value = if negative { -magnitude } else { magnitude };
    Ok(ColumnData::Numeric(Some(Numeric::new_with_scale(
        value, scale,
    ))))
}

/// Encodes `value` as a sql_variant: a u32 total length, then the base
/// type, property bytes and value, mirroring [`decode`]. Any NULL is a zero
/// total length. Ported from upstream Tiberius (tiberius-rs/tiberius
/// dd19eeb). Fork changes: an over-long value is an error rather than a
/// wrapped length, and strings carry a real collation (see the String arm;
/// upstream's zero collation is rejected by the server).
pub(crate) fn encode(dst: &mut bytes::BytesMut, value: ColumnData<'_>) -> crate::Result<()> {
    use bytes::BufMut;

    let too_long = |what: &str, n: usize| {
        Error::Conversion(
            format!(
                "sql_variant: {what} of {n} bytes exceeds the {MAX_VARIANT_PAYLOAD}-byte limit"
            )
            .into(),
        )
    };

    let mut body = bytes::BytesMut::new();
    let fixed = |body: &mut bytes::BytesMut, ty: FixedLenType| {
        body.put_u8(ty as u8);
        body.put_u8(0);
    };

    match value {
        ColumnData::Bit(Some(v)) => {
            fixed(&mut body, FixedLenType::Bit);
            body.put_u8(v as u8);
        }
        ColumnData::U8(Some(v)) => {
            fixed(&mut body, FixedLenType::Int1);
            body.put_u8(v);
        }
        ColumnData::I16(Some(v)) => {
            fixed(&mut body, FixedLenType::Int2);
            body.put_i16_le(v);
        }
        ColumnData::I32(Some(v)) => {
            fixed(&mut body, FixedLenType::Int4);
            body.put_i32_le(v);
        }
        ColumnData::I64(Some(v)) => {
            fixed(&mut body, FixedLenType::Int8);
            body.put_i64_le(v);
        }
        ColumnData::F32(Some(v)) => {
            fixed(&mut body, FixedLenType::Float4);
            body.put_f32_le(v);
        }
        ColumnData::F64(Some(v)) => {
            fixed(&mut body, FixedLenType::Float8);
            body.put_f64_le(v);
        }
        ColumnData::DateTime(Some(dt)) => {
            fixed(&mut body, FixedLenType::Datetime);
            crate::tds::codec::Encode::encode(dt, &mut body)?;
        }
        ColumnData::SmallDateTime(Some(dt)) => {
            fixed(&mut body, FixedLenType::Datetime4);
            crate::tds::codec::Encode::encode(dt, &mut body)?;
        }
        ColumnData::Guid(Some(uuid)) => {
            body.put_u8(VarLenType::Guid as u8);
            body.put_u8(0);
            let mut bytes = *uuid.as_bytes();
            crate::tds::codec::guid::reorder_bytes(&mut bytes);
            body.extend_from_slice(&bytes);
        }
        ColumnData::Numeric(Some(num)) => {
            body.put_u8(VarLenType::Numericn as u8);
            body.put_u8(2);
            body.put_u8(num.precision());
            body.put_u8(num.scale());
            // Numeric's own encoding starts with a length byte, which a
            // variant value doesn't have.
            let mut tmp = bytes::BytesMut::new();
            crate::tds::codec::Encode::encode(num, &mut tmp)?;
            body.extend_from_slice(&tmp[1..]);
        }
        ColumnData::String(Some(s)) => {
            let utf16: Vec<u8> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
            if utf16.len() > MAX_VARIANT_PAYLOAD {
                return Err(too_long("string", utf16.len()));
            }
            body.put_u8(VarLenType::NVarchar as u8);
            body.put_u8(7);
            // A real collation is required: SQL Server rejects upstream's
            // all-zero one with error 8001 ("Meta-information is invalid
            // for the Sql Variant parameter"), found by live testing. For
            // nvarchar it only governs comparisons, not storage, so send
            // SQL_Latin1_General_CP1_CI_AS (LCID 0x0409, flags 0xD0, sort
            // ID 52), SQL Server's own default.
            body.put_u32_le(0x00D0_0409);
            body.put_u8(52);
            body.put_u16_le(MAX_VARIANT_PAYLOAD as u16);
            body.extend_from_slice(&utf16);
        }
        ColumnData::Binary(Some(b)) => {
            if b.len() > MAX_VARIANT_PAYLOAD {
                return Err(too_long("binary value", b.len()));
            }
            body.put_u8(VarLenType::BigVarBin as u8);
            body.put_u8(2);
            body.put_u16_le(MAX_VARIANT_PAYLOAD as u16);
            body.extend_from_slice(&b);
        }
        #[cfg(feature = "tds73")]
        ColumnData::Date(Some(date)) => {
            body.put_u8(VarLenType::Daten as u8);
            body.put_u8(0);
            crate::tds::codec::Encode::encode(date, &mut body)?;
        }
        #[cfg(feature = "tds73")]
        ColumnData::Time(Some(time)) => {
            body.put_u8(VarLenType::Timen as u8);
            body.put_u8(1);
            body.put_u8(time.scale());
            crate::tds::codec::Encode::encode(time, &mut body)?;
        }
        #[cfg(feature = "tds73")]
        ColumnData::DateTime2(Some(dt)) => {
            body.put_u8(VarLenType::Datetime2 as u8);
            body.put_u8(1);
            body.put_u8(dt.time().scale());
            crate::tds::codec::Encode::encode(dt, &mut body)?;
        }
        #[cfg(feature = "tds73")]
        ColumnData::DateTimeOffset(Some(dto)) => {
            body.put_u8(VarLenType::DatetimeOffsetn as u8);
            body.put_u8(1);
            body.put_u8(dto.datetime2().time().scale());
            crate::tds::codec::Encode::encode(dto, &mut body)?;
        }
        ColumnData::Xml(Some(_)) => {
            return Err(Error::Conversion(
                "sql_variant: xml is not a valid sql_variant base type".into(),
            ))
        }
        _ => {
            dst.put_u32_le(0); // a NULL variant
            return Ok(());
        }
    }

    dst.put_u32_le(body.len() as u32);
    dst.extend_from_slice(&body);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::decode;
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    use crate::ColumnData;
    use bytes::{BufMut, BytesMut};

    async fn run(bytes: &[u8]) -> crate::Result<ColumnData<'static>> {
        decode(&mut BytesMut::from(bytes).into_sql_read_bytes()).await
    }

    fn variant(base: u8, props: &[u8], value: &[u8]) -> Vec<u8> {
        let mut b = BytesMut::new();
        b.put_u32_le((2 + props.len() + value.len()) as u32);
        b.put_u8(base);
        b.put_u8(props.len() as u8);
        b.put_slice(props);
        b.put_slice(value);
        b.to_vec()
    }

    #[tokio::test]
    async fn null_int_and_nvarchar() {
        assert_eq!(run(&[0, 0, 0, 0]).await.unwrap(), ColumnData::String(None));
        assert_eq!(
            run(&variant(0x38, &[], &7i32.to_le_bytes())).await.unwrap(),
            ColumnData::I32(Some(7))
        );
        let text: Vec<u8> = "hé".encode_utf16().flat_map(u16::to_le_bytes).collect();
        let props = [0x09, 0x04, 0xd0, 0x00, 0x34, 0x10, 0x00];
        assert_eq!(
            run(&variant(0xE7, &props, &text)).await.unwrap(),
            ColumnData::String(Some("hé".into()))
        );
    }

    // Every encodable value decodes back to itself.
    #[tokio::test]
    async fn encode_decode_round_trip() {
        use crate::tds::Numeric;
        for value in [
            ColumnData::I32(Some(-7)),
            ColumnData::I64(Some(9_000_000_000)),
            ColumnData::U8(Some(200)),
            ColumnData::I16(Some(-300)),
            ColumnData::Bit(Some(true)),
            ColumnData::F64(Some(1.25)),
            ColumnData::F32(Some(0.5)),
            ColumnData::Numeric(Some(Numeric::new_with_scale(-12345, 2))),
            ColumnData::String(Some("héllo".into())),
            ColumnData::Binary(Some((&[1u8, 2, 3][..]).into())),
            ColumnData::Guid(Some(uuid::Uuid::from_bytes([7; 16]))),
            ColumnData::String(None),
        ] {
            let mut buf = BytesMut::new();
            super::encode(&mut buf, value.clone()).unwrap();
            assert_eq!(run(&buf).await.unwrap(), value, "{value:?}");
        }
        let mut buf = BytesMut::new();
        assert!(
            super::encode(&mut buf, ColumnData::String(Some("x".repeat(4001).into()))).is_err()
        );
    }

    #[tokio::test]
    async fn guid_has_no_length_byte() {
        let v = run(&variant(0x24, &[], &[0x11; 16])).await.unwrap();
        assert!(matches!(v, ColumnData::Guid(Some(_))), "{v:?}");
    }

    #[tokio::test]
    async fn numeric_value() {
        let mut v = vec![0u8]; // negative
        v.extend_from_slice(&12345u32.to_le_bytes());
        let d = run(&variant(0x6C, &[10, 2], &v)).await.unwrap();
        let ColumnData::Numeric(Some(n)) = d else {
            panic!("{d:?}")
        };
        assert_eq!((n.value(), n.scale()), (-12345, 2));
    }

    #[tokio::test]
    async fn malformed_variants_are_protocol_errors_not_desyncs() {
        for bytes in [
            variant(0x38, &[], &[1, 2]),                // int with 2 value bytes
            variant(0x38, &[0], &[1, 2, 3, 4]),         // int with property bytes
            variant(0x6C, &[10, 39], &[1, 0, 0, 0, 0]), // scale > precision
            variant(
                0x6C,
                &[38, 0],
                &[
                    1, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xff, 0xff,
                ],
            ), // > i128::MAX
            variant(0x24, &[], &[0; 4]),                // guid with 4 bytes
            variant(0xF1, &[], &[]),                    // xml: not a valid base type
            vec![1, 0, 0, 0, 0x38],                     // total length 1
        ] {
            let err = run(&bytes).await.unwrap_err();
            assert!(
                matches!(err, crate::Error::Protocol(_)),
                "{bytes:?}: {err:?}"
            );
        }
    }
}
