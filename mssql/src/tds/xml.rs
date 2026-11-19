//! The XML containers
use super::codec::Encode;
use bytes::{BufMut, BytesMut};
use std::borrow::BorrowMut;
use std::sync::Arc;

/// Provides information of the location for the schema.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct XmlSchema {
    db_name: String,
    owner: String,
    collection: String,
}

impl XmlSchema {
    pub(crate) fn new(
        db_name: impl ToString,
        owner: impl ToString,
        collection: impl ToString,
    ) -> Self {
        Self {
            db_name: db_name.to_string(),
            owner: owner.to_string(),
            collection: collection.to_string(),
        }
    }

    /// Specifies the name of the database where the schema collection is defined.
    pub fn db_name(&self) -> &str {
        &self.db_name
    }

    /// Specifies the name of the relational schema containing the schema collection.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Specifies the name of the XML schema collection to which the type is
    /// bound.
    pub fn collection(&self) -> &str {
        &self.collection
    }
}

/// A representation of XML data in TDS. Holds the data as a UTF-8 string and
/// and optional information about the schema.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct XmlData {
    data: String,
    schema: Option<Arc<XmlSchema>>,
}

impl XmlData {
    /// Create a new XmlData with the given string. Validation of the XML data
    /// happens in the database.
    pub fn new(data: impl ToString) -> Self {
        Self {
            data: data.to_string(),
            schema: None,
        }
    }

    pub(crate) fn set_schema(&mut self, schema: Arc<XmlSchema>) {
        self.schema = Some(schema);
    }

    /// Returns information about the schema of the XML file, if existing.
    #[allow(clippy::option_as_ref_deref)]
    pub fn schema(&self) -> Option<&XmlSchema> {
        self.schema.as_ref().map(|s| &**s)
    }

    /// Takes the XML string out from the struct.
    pub fn into_string(self) -> String {
        self.data
    }
}

impl std::fmt::Display for XmlData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.data)
    }
}

impl AsRef<str> for XmlData {
    fn as_ref(&self) -> &str {
        self.data.as_ref()
    }
}

impl Encode<BytesMut> for XmlData {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        // unknown size
        dst.put_u64_le(0xfffffffffffffffe_u64);

        // first blob
        let mut length = 0u32;
        let len_pos = dst.len();

        // writing the length later
        dst.put_u32_le(length);

        for chr in self.data.encode_utf16() {
            length += 1;
            dst.put_u16_le(chr);
        }

        // PLP_TERMINATOR, no next blobs
        dst.put_u32_le(0);

        let dst: &mut [u8] = dst.borrow_mut();
        let mut dst = &mut dst[len_pos..];
        dst.put_u32_le(length * 2);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{XmlData, XmlSchema};
    use crate::tds::codec::Encode;
    use bytes::BytesMut;
    use std::sync::Arc;

    #[test]
    fn schema_accessors_and_xml_views() {
        let schema = Arc::new(XmlSchema::new("catalog", "dbo", "widgets"));
        assert_eq!("catalog", schema.db_name());
        assert_eq!("dbo", schema.owner());
        assert_eq!("widgets", schema.collection());

        let mut xml = XmlData::new("<widget />");
        assert_eq!(None, xml.schema());
        xml.set_schema(schema);
        assert_eq!(Some("widgets"), xml.schema().map(XmlSchema::collection));
        assert_eq!("<widget />", xml.as_ref());
        assert_eq!("<widget />", xml.to_string());
        assert_eq!("<widget />", xml.into_string());
    }

    #[test]
    fn encoding_uses_plp_with_utf16_byte_length() {
        let mut bytes = BytesMut::new();
        XmlData::new("a🦀").encode(&mut bytes).unwrap();

        assert_eq!(
            &[0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
            &bytes[..8]
        );
        assert_eq!(&[6, 0, 0, 0], &bytes[8..12]);
        assert_eq!(&[0, 0, 0, 0], &bytes[18..22]);
    }
}
