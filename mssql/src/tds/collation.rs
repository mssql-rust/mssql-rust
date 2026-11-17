//! legacy implementation of collations (or codepages rather) for dealing with varchar's with legacy databases
//! references [1] which has some mappings from the katmai (SQL Server 2008) source code and is a TDS driver
//! directly from microsoft
//! [2] is helpful to map CP1234 to the appropriate encoding
//!
//! [1] <https://github.com/Microsoft/mssql-jdbc/blob/eb14f63077c47ef1fc1c690deb8cfab602baeb85/src/main/java/com/microsoft/sqlserver/jdbc/SQLCollation.java>
//! [2] <https://github.com/lifthrasiir/rust-encoding/blob/496823171f15d9b9446b2ec3fb7765f22346256b/src/label.rs#L282>
//!
//! The CP437/CP850 high-byte tables follow the Unicode Consortium mappings
//! <https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/PC/CP437.TXT> and
//! <https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/PC/CP850.TXT>
//! (checked byte for byte against those files). Ported from upstream Tiberius
//! (tiberius-rs/tiberius 30f7b36, by t8y2). Thanks to the Tiberius team.

use encoding_rs::Encoding;
use std::fmt;

use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Collation {
    /// LCID ColFlags Version
    info: u32,
    /// Sortid
    sort_id: u8,
}

impl Collation {
    pub fn new(info: u32, sort_id: u8) -> Self {
        Self { info, sort_id }
    }

    /// return the locale id part of the LCID (the specification here uses ambiguous terms)
    pub fn lcid(&self) -> u16 {
        (self.info & 0xffff) as u16
    }

    pub fn sort_id(&self) -> u8 {
        self.sort_id
    }

    pub fn info(&self) -> u32 {
        self.info
    }

    /// return an encoding for a given collation
    pub fn encoding(&self) -> crate::Result<&'static Encoding> {
        let res = if self.sort_id == 0 {
            lcid_to_encoding(self.lcid())
        } else {
            sortid_to_encoding(self.sort_id)
        };

        res.ok_or_else(|| {
            Error::Encoding(
                format!(
                    "encoding: unspported encoding (LCID: {:#06x}, sort ID: {})",
                    self.lcid(),
                    self.sort_id(),
                )
                .into(),
            )
        })
    }
}

impl Collation {
    /// The codec used for `char`/`varchar`/`text` data in this collation.
    /// Unlike [`encoding`](Self::encoding), it also covers the legacy DOS
    /// code pages CP437 (sort IDs 30–35) and CP850 (40–45, 49, 55–61), which
    /// `encoding_rs` doesn't implement. Those collations used to fail every
    /// read and write with "unsupported encoding".
    pub(crate) fn codec(&self) -> crate::Result<CollationCodec> {
        match self.sort_id {
            30..=35 => Ok(CollationCodec::Cp437),
            40..=45 | 49 | 55..=61 => Ok(CollationCodec::Cp850),
            _ => self.encoding().map(CollationCodec::BuiltIn),
        }
    }
}

impl fmt::Display for Collation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.codec() {
            Ok(codec) => write!(f, "{}", codec.name()),
            _ => write!(f, "None"),
        }
    }
}

const CP437_HIGH: [char; 128] = [
    '\u{c7}', '\u{fc}', '\u{e9}', '\u{e2}', '\u{e4}', '\u{e0}', '\u{e5}', '\u{e7}', '\u{ea}',
    '\u{eb}', '\u{e8}', '\u{ef}', '\u{ee}', '\u{ec}', '\u{c4}', '\u{c5}', '\u{c9}', '\u{e6}',
    '\u{c6}', '\u{f4}', '\u{f6}', '\u{f2}', '\u{fb}', '\u{f9}', '\u{ff}', '\u{d6}', '\u{dc}',
    '\u{a2}', '\u{a3}', '\u{a5}', '\u{20a7}', '\u{192}', '\u{e1}', '\u{ed}', '\u{f3}', '\u{fa}',
    '\u{f1}', '\u{d1}', '\u{aa}', '\u{ba}', '\u{bf}', '\u{2310}', '\u{ac}', '\u{bd}', '\u{bc}',
    '\u{a1}', '\u{ab}', '\u{bb}', '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}',
    '\u{2561}', '\u{2562}', '\u{2556}', '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255d}',
    '\u{255c}', '\u{255b}', '\u{2510}', '\u{2514}', '\u{2534}', '\u{252c}', '\u{251c}', '\u{2500}',
    '\u{253c}', '\u{255e}', '\u{255f}', '\u{255a}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}',
    '\u{2550}', '\u{256c}', '\u{2567}', '\u{2568}', '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}',
    '\u{2552}', '\u{2553}', '\u{256b}', '\u{256a}', '\u{2518}', '\u{250c}', '\u{2588}', '\u{2584}',
    '\u{258c}', '\u{2590}', '\u{2580}', '\u{3b1}', '\u{df}', '\u{393}', '\u{3c0}', '\u{3a3}',
    '\u{3c3}', '\u{b5}', '\u{3c4}', '\u{3a6}', '\u{398}', '\u{3a9}', '\u{3b4}', '\u{221e}',
    '\u{3c6}', '\u{3b5}', '\u{2229}', '\u{2261}', '\u{b1}', '\u{2265}', '\u{2264}', '\u{2320}',
    '\u{2321}', '\u{f7}', '\u{2248}', '\u{b0}', '\u{2219}', '\u{b7}', '\u{221a}', '\u{207f}',
    '\u{b2}', '\u{25a0}', '\u{a0}',
];

const CP850_HIGH: [char; 128] = [
    '\u{c7}', '\u{fc}', '\u{e9}', '\u{e2}', '\u{e4}', '\u{e0}', '\u{e5}', '\u{e7}', '\u{ea}',
    '\u{eb}', '\u{e8}', '\u{ef}', '\u{ee}', '\u{ec}', '\u{c4}', '\u{c5}', '\u{c9}', '\u{e6}',
    '\u{c6}', '\u{f4}', '\u{f6}', '\u{f2}', '\u{fb}', '\u{f9}', '\u{ff}', '\u{d6}', '\u{dc}',
    '\u{f8}', '\u{a3}', '\u{d8}', '\u{d7}', '\u{192}', '\u{e1}', '\u{ed}', '\u{f3}', '\u{fa}',
    '\u{f1}', '\u{d1}', '\u{aa}', '\u{ba}', '\u{bf}', '\u{ae}', '\u{ac}', '\u{bd}', '\u{bc}',
    '\u{a1}', '\u{ab}', '\u{bb}', '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}',
    '\u{c1}', '\u{c2}', '\u{c0}', '\u{a9}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255d}',
    '\u{a2}', '\u{a5}', '\u{2510}', '\u{2514}', '\u{2534}', '\u{252c}', '\u{251c}', '\u{2500}',
    '\u{253c}', '\u{e3}', '\u{c3}', '\u{255a}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}',
    '\u{2550}', '\u{256c}', '\u{a4}', '\u{f0}', '\u{d0}', '\u{ca}', '\u{cb}', '\u{c8}', '\u{131}',
    '\u{cd}', '\u{ce}', '\u{cf}', '\u{2518}', '\u{250c}', '\u{2588}', '\u{2584}', '\u{a6}',
    '\u{cc}', '\u{2580}', '\u{d3}', '\u{df}', '\u{d4}', '\u{d2}', '\u{f5}', '\u{d5}', '\u{b5}',
    '\u{fe}', '\u{de}', '\u{da}', '\u{db}', '\u{d9}', '\u{fd}', '\u{dd}', '\u{af}', '\u{b4}',
    '\u{ad}', '\u{b1}', '\u{2017}', '\u{be}', '\u{b6}', '\u{a7}', '\u{f7}', '\u{b8}', '\u{b0}',
    '\u{a8}', '\u{b7}', '\u{b9}', '\u{b3}', '\u{b2}', '\u{25a0}', '\u{a0}',
];

pub(crate) enum CollationCodec {
    BuiltIn(&'static Encoding),
    Cp437,
    Cp850,
}

impl CollationCodec {
    fn name(&self) -> &'static str {
        match self {
            Self::BuiltIn(encoding) => encoding.name(),
            Self::Cp437 => "CP437",
            Self::Cp850 => "CP850",
        }
    }

    /// Strict decode: an invalid sequence is an error. (CP437/CP850 map
    /// every byte, so only built-in encodings can fail.)
    pub(crate) fn decode(&self, bytes: &[u8]) -> crate::Result<String> {
        match self {
            Self::BuiltIn(encoding) => encoding
                .decode_without_bom_handling_and_without_replacement(bytes)
                .map(|s| s.into_owned())
                .ok_or_else(|| Error::Encoding("invalid sequence".into())),
            Self::Cp437 => Ok(decode_single_byte(bytes, &CP437_HIGH)),
            Self::Cp850 => Ok(decode_single_byte(bytes, &CP850_HIGH)),
        }
    }

    /// Lenient decode: invalid sequences become U+FFFD. With `lossy`
    /// false this is [`decode`](Self::decode).
    pub(crate) fn decode_with(&self, bytes: &[u8], lossy: bool) -> crate::Result<String> {
        match (self, lossy) {
            (Self::BuiltIn(encoding), true) => {
                Ok(encoding.decode_without_bom_handling(bytes).0.into_owned())
            }
            _ => self.decode(bytes),
        }
    }

    /// Strict encode: a character the code page can't represent is an error
    /// naming it.
    pub(crate) fn encode(&self, text: &str) -> crate::Result<Vec<u8>> {
        let unrepresentable =
            |c: char| Error::Encoding(format!("unrepresentable character: {c}").into());

        match self {
            Self::BuiltIn(encoding) => {
                let mut encoder = encoding.new_encoder();
                let capacity = encoder
                    .max_buffer_length_from_utf8_without_replacement(text.len())
                    .ok_or_else(|| Error::Encoding("string too long to encode".into()))?;
                let mut bytes = Vec::with_capacity(capacity);
                let (result, _) =
                    encoder.encode_from_utf8_to_vec_without_replacement(text, &mut bytes, true);
                match result {
                    encoding_rs::EncoderResult::InputEmpty => Ok(bytes),
                    encoding_rs::EncoderResult::Unmappable(c) => Err(unrepresentable(c)),
                    encoding_rs::EncoderResult::OutputFull => {
                        Err(Error::Encoding("encoder output buffer full".into()))
                    }
                }
            }
            Self::Cp437 => encode_single_byte(text, &CP437_HIGH).map_err(unrepresentable),
            Self::Cp850 => encode_single_byte(text, &CP850_HIGH).map_err(unrepresentable),
        }
    }
}

fn decode_single_byte(bytes: &[u8], high: &[char; 128]) -> String {
    bytes
        .iter()
        .map(|&byte| match byte {
            0..=0x7f => char::from(byte),
            _ => high[usize::from(byte) - 0x80],
        })
        .collect()
}

fn encode_single_byte(text: &str, high: &[char; 128]) -> Result<Vec<u8>, char> {
    let mut bytes = Vec::with_capacity(text.len());
    for character in text.chars() {
        let byte = match character {
            '\0'..='\u{7f}' => character as u8,
            _ => {
                high.iter()
                    .position(|&mapped| mapped == character)
                    .ok_or(character)? as u8
                    + 0x80
            }
        };
        bytes.push(byte);
    }
    Ok(bytes)
}

/// https://github.com/Microsoft/mssql-jdbc/blob/eb14f63077c47ef1fc1c690deb8cfab602baeb85/src/main/java/com/microsoft/sqlserver/jdbc/SQLCollation.java#L102-L310
/// maps an LCID (it's locale part which is only 2 bytes) to a codepage
///
/// generate the code below from source code:
/// 1. (regex)replace: (.*?)\((.*?),(.*?)\) with $2 => $3
/// 2. replace: Encoding.CP(.*?) with encoding::all::WINDOWS_$1
/// 3. replace: Encoding.UNICODE with encoding::all::UTF16_LE
//
///    the unimplemented!() one's are not supported by rust-encoding
pub fn lcid_to_encoding(locale: u16) -> Option<&'static Encoding> {
    match locale {
        0x0401 => Some(encoding_rs::WINDOWS_1256),
        0x0402 => Some(encoding_rs::WINDOWS_1251),
        0x0403 => Some(encoding_rs::WINDOWS_1252),
        // CP950
        0x0404 | 0x0c04 | 0x1404 => Some(encoding_rs::BIG5),
        0x0405 => Some(encoding_rs::WINDOWS_1250),
        0x0406 => Some(encoding_rs::WINDOWS_1252),
        0x0407 => Some(encoding_rs::WINDOWS_1252),
        0x0408 => Some(encoding_rs::WINDOWS_1253),
        0x0409 => Some(encoding_rs::WINDOWS_1252),
        0x040a => Some(encoding_rs::WINDOWS_1252),
        0x040b => Some(encoding_rs::WINDOWS_1252),
        0x040c => Some(encoding_rs::WINDOWS_1252),
        0x040d => Some(encoding_rs::WINDOWS_1255),
        0x040e => Some(encoding_rs::WINDOWS_1250),
        0x040f => Some(encoding_rs::WINDOWS_1252),
        0x0410 => Some(encoding_rs::WINDOWS_1252),
        // CP932
        0x0411 => Some(encoding_rs::SHIFT_JIS),
        0x0412 => Some(encoding_rs::EUC_KR),
        0x0413 => Some(encoding_rs::WINDOWS_1252),
        0x0414 => Some(encoding_rs::WINDOWS_1252),
        0x0415 => Some(encoding_rs::WINDOWS_1250),
        0x0416 => Some(encoding_rs::WINDOWS_1252),
        0x0417 => Some(encoding_rs::WINDOWS_1252),
        0x0418 => Some(encoding_rs::WINDOWS_1250),
        0x0419 => Some(encoding_rs::WINDOWS_1251),
        0x041a => Some(encoding_rs::WINDOWS_1250),
        0x041b => Some(encoding_rs::WINDOWS_1250),
        0x041c => Some(encoding_rs::WINDOWS_1250),
        0x041d => Some(encoding_rs::WINDOWS_1252),
        0x041e => Some(encoding_rs::WINDOWS_874),
        0x041f => Some(encoding_rs::WINDOWS_1254),
        0x0420 => Some(encoding_rs::WINDOWS_1256),
        0x0421 => Some(encoding_rs::WINDOWS_1252),
        0x0422 => Some(encoding_rs::WINDOWS_1251),
        0x0423 => Some(encoding_rs::WINDOWS_1251),
        0x0424 => Some(encoding_rs::WINDOWS_1250),
        0x0425 => Some(encoding_rs::WINDOWS_1257),
        0x0426 => Some(encoding_rs::WINDOWS_1257),
        0x0427 => Some(encoding_rs::WINDOWS_1257),
        0x0428 => Some(encoding_rs::WINDOWS_1251),
        0x0429 => Some(encoding_rs::WINDOWS_1256),
        0x042a => Some(encoding_rs::WINDOWS_1258),
        0x042b => Some(encoding_rs::WINDOWS_1252),
        0x042c => Some(encoding_rs::WINDOWS_1254),
        0x042d => Some(encoding_rs::WINDOWS_1252),
        0x042e => Some(encoding_rs::WINDOWS_1252),
        0x042f => Some(encoding_rs::WINDOWS_1251),
        0x0432 => Some(encoding_rs::WINDOWS_1252),
        0x0434 => Some(encoding_rs::WINDOWS_1252),
        0x0435 => Some(encoding_rs::WINDOWS_1252),
        0x0436 => Some(encoding_rs::WINDOWS_1252),
        0x0437 => Some(encoding_rs::WINDOWS_1252),
        0x0438 => Some(encoding_rs::WINDOWS_1252),
        0x0439 => Some(encoding_rs::UTF_16LE),
        0x043a => Some(encoding_rs::UTF_16LE),
        0x043b => Some(encoding_rs::WINDOWS_1252),
        0x043e => Some(encoding_rs::WINDOWS_1252),
        0x043f => Some(encoding_rs::WINDOWS_1251),
        0x0440 => Some(encoding_rs::WINDOWS_1251),
        0x0441 => Some(encoding_rs::WINDOWS_1252),
        0x0442 => Some(encoding_rs::WINDOWS_1250),
        0x0443 => Some(encoding_rs::WINDOWS_1254),
        0x0444 => Some(encoding_rs::WINDOWS_1251),
        0x0445 => Some(encoding_rs::UTF_16LE),
        0x0446 => Some(encoding_rs::UTF_16LE),
        0x0447 => Some(encoding_rs::UTF_16LE),
        0x0448 => Some(encoding_rs::UTF_16LE),
        0x0449 => Some(encoding_rs::UTF_16LE),
        0x044a => Some(encoding_rs::UTF_16LE),
        0x044b => Some(encoding_rs::UTF_16LE),
        0x044c => Some(encoding_rs::UTF_16LE),
        0x044d => Some(encoding_rs::UTF_16LE),
        0x044e => Some(encoding_rs::UTF_16LE),
        0x044f => Some(encoding_rs::UTF_16LE),
        0x0450 => Some(encoding_rs::WINDOWS_1251),
        0x0451 => Some(encoding_rs::UTF_16LE),
        0x0452 => Some(encoding_rs::WINDOWS_1252),
        0x0453 => Some(encoding_rs::UTF_16LE),
        0x0454 => Some(encoding_rs::UTF_16LE),
        0x0456 => Some(encoding_rs::WINDOWS_1252),
        0x0457 => Some(encoding_rs::UTF_16LE),
        0x045a => Some(encoding_rs::UTF_16LE),
        0x045b => Some(encoding_rs::UTF_16LE),
        0x045d => Some(encoding_rs::WINDOWS_1252),
        0x045e => Some(encoding_rs::WINDOWS_1252),
        0x0461 => Some(encoding_rs::UTF_16LE),
        0x0462 => Some(encoding_rs::WINDOWS_1252),
        0x0463 => Some(encoding_rs::UTF_16LE),
        0x0464 => Some(encoding_rs::WINDOWS_1252),
        0x0465 => Some(encoding_rs::UTF_16LE),
        0x0468 => Some(encoding_rs::WINDOWS_1252),
        0x046a => Some(encoding_rs::WINDOWS_1252),
        0x046b => Some(encoding_rs::WINDOWS_1252),
        0x046c => Some(encoding_rs::WINDOWS_1252),
        0x046d => Some(encoding_rs::WINDOWS_1251),
        0x046e => Some(encoding_rs::WINDOWS_1252),
        0x046f => Some(encoding_rs::WINDOWS_1252),
        0x0470 => Some(encoding_rs::WINDOWS_1252),
        0x0478 => Some(encoding_rs::WINDOWS_1252),
        0x047a => Some(encoding_rs::WINDOWS_1252),
        0x047c => Some(encoding_rs::WINDOWS_1252),
        0x047e => Some(encoding_rs::WINDOWS_1252),
        0x0480 => Some(encoding_rs::WINDOWS_1256),
        0x0481 => Some(encoding_rs::UTF_16LE),
        0x0482 => Some(encoding_rs::WINDOWS_1252),
        0x0483 => Some(encoding_rs::WINDOWS_1252),
        0x0484 => Some(encoding_rs::WINDOWS_1252),
        0x0485 => Some(encoding_rs::WINDOWS_1251),
        0x0486 => Some(encoding_rs::WINDOWS_1252),
        0x0487 => Some(encoding_rs::WINDOWS_1252),
        0x0488 => Some(encoding_rs::WINDOWS_1252),
        0x048c => Some(encoding_rs::WINDOWS_1256),
        0x0801 => Some(encoding_rs::WINDOWS_1256),
        // CP936
        0x0804 | 0x1004 => Some(encoding_rs::GB18030),
        0x0807 => Some(encoding_rs::WINDOWS_1252),
        0x0809 => Some(encoding_rs::WINDOWS_1252),
        0x080a => Some(encoding_rs::WINDOWS_1252),
        0x080c => Some(encoding_rs::WINDOWS_1252),
        0x0810 => Some(encoding_rs::WINDOWS_1252),
        0x0813 => Some(encoding_rs::WINDOWS_1252),
        0x0814 => Some(encoding_rs::WINDOWS_1252),
        0x0816 => Some(encoding_rs::WINDOWS_1252),
        0x081a => Some(encoding_rs::WINDOWS_1250),
        0x081d => Some(encoding_rs::WINDOWS_1252),
        0x0827 => Some(encoding_rs::WINDOWS_1257),
        0x082c => Some(encoding_rs::WINDOWS_1251),
        0x082e => Some(encoding_rs::WINDOWS_1252),
        0x083b => Some(encoding_rs::WINDOWS_1252),
        0x083c => Some(encoding_rs::WINDOWS_1252),
        0x083e => Some(encoding_rs::WINDOWS_1252),
        0x0843 => Some(encoding_rs::WINDOWS_1251),
        0x0845 => Some(encoding_rs::UTF_16LE),
        0x0850 => Some(encoding_rs::WINDOWS_1251),
        0x085d => Some(encoding_rs::WINDOWS_1252),
        0x085f => Some(encoding_rs::WINDOWS_1252),
        0x086b => Some(encoding_rs::WINDOWS_1252),
        0x0c01 => Some(encoding_rs::WINDOWS_1256),
        0x0c07 => Some(encoding_rs::WINDOWS_1252),
        0x0c09 => Some(encoding_rs::WINDOWS_1252),
        0x0c0a => Some(encoding_rs::WINDOWS_1252),
        0x0c0c => Some(encoding_rs::WINDOWS_1252),
        0x0c1a => Some(encoding_rs::WINDOWS_1251),
        0x0c3b => Some(encoding_rs::WINDOWS_1252),
        0x0c6b => Some(encoding_rs::WINDOWS_1252),
        0x1001 => Some(encoding_rs::WINDOWS_1256),
        0x1007 => Some(encoding_rs::WINDOWS_1252),
        0x1009 => Some(encoding_rs::WINDOWS_1252),
        0x100a => Some(encoding_rs::WINDOWS_1252),
        0x100c => Some(encoding_rs::WINDOWS_1252),
        0x101a => Some(encoding_rs::WINDOWS_1250),
        0x103b => Some(encoding_rs::WINDOWS_1252),
        0x1401 => Some(encoding_rs::WINDOWS_1256),
        0x1407 => Some(encoding_rs::WINDOWS_1252),
        0x1409 => Some(encoding_rs::WINDOWS_1252),
        0x140a => Some(encoding_rs::WINDOWS_1252),
        0x140c => Some(encoding_rs::WINDOWS_1252),
        0x141a => Some(encoding_rs::WINDOWS_1250),
        0x143b => Some(encoding_rs::WINDOWS_1252),
        0x1801 => Some(encoding_rs::WINDOWS_1256),
        0x1809 => Some(encoding_rs::WINDOWS_1252),
        0x180a => Some(encoding_rs::WINDOWS_1252),
        0x180c => Some(encoding_rs::WINDOWS_1252),
        0x181a => Some(encoding_rs::WINDOWS_1250),
        0x183b => Some(encoding_rs::WINDOWS_1252),
        0x1c01 => Some(encoding_rs::WINDOWS_1256),
        0x1c09 => Some(encoding_rs::WINDOWS_1252),
        0x1c0a => Some(encoding_rs::WINDOWS_1252),
        0x1c1a => Some(encoding_rs::WINDOWS_1251),
        0x1c3b => Some(encoding_rs::WINDOWS_1252),
        0x2001 => Some(encoding_rs::WINDOWS_1256),
        0x2009 => Some(encoding_rs::WINDOWS_1252),
        0x200a => Some(encoding_rs::WINDOWS_1252),
        0x201a => Some(encoding_rs::WINDOWS_1251),
        0x203b => Some(encoding_rs::WINDOWS_1252),
        0x2401 => Some(encoding_rs::WINDOWS_1256),
        0x2409 => Some(encoding_rs::WINDOWS_1252),
        0x240a => Some(encoding_rs::WINDOWS_1252),
        0x243b => Some(encoding_rs::WINDOWS_1252),
        0x2801 => Some(encoding_rs::WINDOWS_1256),
        0x2809 => Some(encoding_rs::WINDOWS_1252),
        0x280a => Some(encoding_rs::WINDOWS_1252),
        0x2c01 => Some(encoding_rs::WINDOWS_1256),
        0x2c09 => Some(encoding_rs::WINDOWS_1252),
        0x2c0a => Some(encoding_rs::WINDOWS_1252),
        0x3001 => Some(encoding_rs::WINDOWS_1256),
        0x3009 => Some(encoding_rs::WINDOWS_1252),
        0x300a => Some(encoding_rs::WINDOWS_1252),
        0x3401 => Some(encoding_rs::WINDOWS_1256),
        0x3409 => Some(encoding_rs::WINDOWS_1252),
        0x340a => Some(encoding_rs::WINDOWS_1252),
        0x3801 => Some(encoding_rs::WINDOWS_1256),
        0x380a => Some(encoding_rs::WINDOWS_1252),
        0x3c01 => Some(encoding_rs::WINDOWS_1256),
        0x3c0a => Some(encoding_rs::WINDOWS_1252),
        0x4001 => Some(encoding_rs::WINDOWS_1256),
        0x4009 => Some(encoding_rs::WINDOWS_1252),
        0x400a => Some(encoding_rs::WINDOWS_1252),
        0x4409 => Some(encoding_rs::WINDOWS_1252),
        0x440a => Some(encoding_rs::WINDOWS_1252),
        0x4809 => Some(encoding_rs::WINDOWS_1252),
        0x480a => Some(encoding_rs::WINDOWS_1252),
        0x4c0a => Some(encoding_rs::WINDOWS_1252),
        0x500a => Some(encoding_rs::WINDOWS_1252),
        0x540a => Some(encoding_rs::WINDOWS_1252),
        _ => None,
    }
}

/// [1] https://github.com/Microsoft/mssql-jdbc/blob/eb14f63077c47ef1fc1c690deb8cfab602baeb85/src/main/java/com/microsoft/sqlserver/jdbc/SQLCollation.java#L362-L482
/// [2] https://msdn.microsoft.com/de-de/library/ms144250(v=sql.105).aspx
///
/// [2] does only contain 3/4 of the content [1] contains, so the source code is again the better source of information
///
/// generate the code below from source code:
/// 1. (regex)replace .*\((.*?),.*?,(.*?)\) with $1 => $2
/// 2. see above/as above
pub fn sortid_to_encoding(sort_id: u8) -> Option<&'static Encoding> {
    match sort_id {
        // 30 | 31 | 32 | 33 | 34 | 35 => Some(encoding_rs::WINDOWS_437),
        // 40 | 41 | 42 | 43 | 44 | 45 | 49 => Some(encoding_rs::WINDOWS_850),
        50 => Some(encoding_rs::WINDOWS_1252),
        51 => Some(encoding_rs::WINDOWS_1252),
        52 => Some(encoding_rs::WINDOWS_1252),
        53 => Some(encoding_rs::WINDOWS_1252),
        54 => Some(encoding_rs::WINDOWS_1252),
        // 55 | 56 | 57 | 58 | 59 | 60 | 61 => Some(encoding_rs::WINDOWS_850),
        71 => Some(encoding_rs::WINDOWS_1252),
        72 => Some(encoding_rs::WINDOWS_1252),
        73 => Some(encoding_rs::WINDOWS_1252),
        74 => Some(encoding_rs::WINDOWS_1252),
        75 => Some(encoding_rs::WINDOWS_1252),
        80 => Some(encoding_rs::WINDOWS_1250),
        81 => Some(encoding_rs::WINDOWS_1250),
        82 => Some(encoding_rs::WINDOWS_1250),
        83 => Some(encoding_rs::WINDOWS_1250),
        84 => Some(encoding_rs::WINDOWS_1250),
        85 => Some(encoding_rs::WINDOWS_1250),
        86 => Some(encoding_rs::WINDOWS_1250),
        87 => Some(encoding_rs::WINDOWS_1250),
        88 => Some(encoding_rs::WINDOWS_1250),
        89 => Some(encoding_rs::WINDOWS_1250),
        90 => Some(encoding_rs::WINDOWS_1250),
        91 => Some(encoding_rs::WINDOWS_1250),
        92 => Some(encoding_rs::WINDOWS_1250),
        93 => Some(encoding_rs::WINDOWS_1250),
        94 => Some(encoding_rs::WINDOWS_1250),
        95 => Some(encoding_rs::WINDOWS_1250),
        96 => Some(encoding_rs::WINDOWS_1250),
        97 => Some(encoding_rs::WINDOWS_1250),
        98 => Some(encoding_rs::WINDOWS_1250),
        104 => Some(encoding_rs::WINDOWS_1251),
        105 => Some(encoding_rs::WINDOWS_1251),
        106 => Some(encoding_rs::WINDOWS_1251),
        107 => Some(encoding_rs::WINDOWS_1251),
        108 => Some(encoding_rs::WINDOWS_1251),
        112 => Some(encoding_rs::WINDOWS_1253),
        113 => Some(encoding_rs::WINDOWS_1253),
        114 => Some(encoding_rs::WINDOWS_1253),
        120 => Some(encoding_rs::WINDOWS_1253),
        121 => Some(encoding_rs::WINDOWS_1253),
        122 => Some(encoding_rs::WINDOWS_1253),
        124 => Some(encoding_rs::WINDOWS_1253),
        128 => Some(encoding_rs::WINDOWS_1254),
        129 => Some(encoding_rs::WINDOWS_1254),
        130 => Some(encoding_rs::WINDOWS_1254),
        136 => Some(encoding_rs::WINDOWS_1255),
        137 => Some(encoding_rs::WINDOWS_1255),
        138 => Some(encoding_rs::WINDOWS_1255),
        144 => Some(encoding_rs::WINDOWS_1256),
        145 => Some(encoding_rs::WINDOWS_1256),
        146 => Some(encoding_rs::WINDOWS_1256),
        152 => Some(encoding_rs::WINDOWS_1257),
        153 => Some(encoding_rs::WINDOWS_1257),
        154 => Some(encoding_rs::WINDOWS_1257),
        155 => Some(encoding_rs::WINDOWS_1257),
        156 => Some(encoding_rs::WINDOWS_1257),
        157 => Some(encoding_rs::WINDOWS_1257),
        158 => Some(encoding_rs::WINDOWS_1257),
        159 => Some(encoding_rs::WINDOWS_1257),
        160 => Some(encoding_rs::WINDOWS_1257),
        183 => Some(encoding_rs::WINDOWS_1252),
        184 => Some(encoding_rs::WINDOWS_1252),
        185 => Some(encoding_rs::WINDOWS_1252),
        186 => Some(encoding_rs::WINDOWS_1252),
        // CP 932
        192 | 193 | 200 => Some(encoding_rs::SHIFT_JIS),
        194 => Some(encoding_rs::EUC_KR),
        195 => Some(encoding_rs::EUC_KR),
        // CP950
        196 | 197 | 202 => Some(encoding_rs::BIG5),
        // CP936 (GB18030 is an extension of it with more chars), should be backwards-compatible)
        198 | 199 | 203 => Some(encoding_rs::GB18030),
        201 => Some(encoding_rs::BIG5),
        204 => Some(encoding_rs::WINDOWS_874),
        205 => Some(encoding_rs::WINDOWS_874),
        206 => Some(encoding_rs::WINDOWS_874),
        210 => Some(encoding_rs::WINDOWS_1252),
        211 => Some(encoding_rs::WINDOWS_1252),
        212 => Some(encoding_rs::WINDOWS_1252),
        213 => Some(encoding_rs::WINDOWS_1252),
        214 => Some(encoding_rs::WINDOWS_1252),
        215 => Some(encoding_rs::WINDOWS_1252),
        216 => Some(encoding_rs::WINDOWS_1252),
        217 => Some(encoding_rs::WINDOWS_1252),
        _ => None,
    }
}

/* TODO
#[cfg(test)]
mod tests {
    use futures_state_stream::StateStream;
    use tokio::executor::current_thread;
    use crate::tests::new_connection;

    #[test]
    fn select_nvarchar_collation_test() {
        let c1 = new_connection();
        let query = c1.simple_query(
            "select cast(cast(N'cześć' as nvarchar(5)) collate Polish_CI_AI as varchar(5))",
        );
        let mut i = 0;
        {
            let future = query.for_each(|x| {
                let val: &str = x.get(0);
                assert_eq!(val, "cześć");
                i += 1;
                Ok(())
            });
            current_thread::block_on_all(future).unwrap();
        }
        assert_eq!(i, 1);
    }
}
*/

#[cfg(test)]
mod codec_tests {
    use super::*;

    #[test]
    fn cp437_and_cp850_sort_ids_have_a_codec() {
        for (sort_id, name) in [
            (30, "CP437"),
            (35, "CP437"),
            (40, "CP850"),
            (49, "CP850"),
            (61, "CP850"),
        ] {
            let c = Collation::new(0x0409, sort_id);
            assert!(c.encoding().is_err(), "encoding_rs has no {name}");
            assert_eq!(c.codec().unwrap().name(), name);
        }
        assert_eq!(
            Collation::new(0x0409, 52).codec().unwrap().name(),
            "windows-1252"
        );
    }

    #[test]
    fn every_high_byte_round_trips() {
        for codec in [CollationCodec::Cp437, CollationCodec::Cp850] {
            let bytes: Vec<u8> = (0u8..=255).collect();
            let text = codec.decode(&bytes).unwrap();
            assert_eq!(text.chars().count(), 256);
            assert_eq!(codec.encode(&text).unwrap(), bytes);
        }
        // Spot checks against the Unicode tables.
        assert_eq!(CollationCodec::Cp437.decode(&[0x82, 0xe1]).unwrap(), "éß");
        assert_eq!(CollationCodec::Cp850.decode(&[0x9b, 0xd5]).unwrap(), "øı");
    }

    #[test]
    fn unrepresentable_characters_are_errors() {
        let err = CollationCodec::Cp437.encode("a€").unwrap_err();
        assert!(err.to_string().contains('€'), "{err}");
    }

    #[test]
    fn lossy_decode_replaces_invalid_sequences_only_when_asked() {
        // Japanese (LCID 0x0411) is Shift_JIS, where a lone lead byte 0x81
        // at the end is an invalid sequence. (encoding_rs maps every byte of
        // the single-byte windows-125x pages, so those can't fail.)
        let codec = Collation::new(0x0411, 0).codec().unwrap();
        assert!(codec.decode_with(&[b'a', 0x81], false).is_err());
        assert_eq!(codec.decode_with(&[b'a', 0x81], true).unwrap(), "a\u{fffd}");
        // Single-byte DOS code pages map every byte, so nothing changes.
        assert_eq!(
            CollationCodec::Cp437.decode_with(&[0x82], true).unwrap(),
            CollationCodec::Cp437.decode(&[0x82]).unwrap()
        );
    }
}
