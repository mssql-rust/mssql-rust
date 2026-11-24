use super::Encode;
use crate::num::convert;
use byteorder::{LittleEndian, WriteBytesExt};
use bytes::BytesMut;
use enumflags2::{bitflags, BitFlags};
use io::{Cursor, Write};
use std::fmt::Debug;
use std::{borrow::Cow, io};
use zeroize::{Zeroize, Zeroizing};

uint_enum! {
    #[repr(u32)]
    #[derive(PartialOrd, Default)]
    pub enum FeatureLevel {
        SqlServerV7 = 0x70000000,
        SqlServer2000 = 0x71000000,
        SqlServer2000Sp1 = 0x71000001,
        SqlServer2005 = 0x72090002,
        SqlServer2008 = 0x730A0003,
        SqlServer2008R2 = 0x730B0003,
        /// 2012, 2014, 2016
        #[default]
        SqlServerN = 0x74000004,
    }
}

enum_into_int!(FeatureLevel => u32);

/// The error for a LOGIN7 message whose offsets or lengths do not fit their
/// wire fields.
fn login_too_large() -> crate::Error {
    crate::Error::Conversion("login_length: value does not fit the target type".into())
}

impl FeatureLevel {
    pub fn done_row_count_bytes(self) -> u8 {
        if self.discriminant() >= FeatureLevel::SqlServer2005.discriminant() {
            8
        } else {
            4
        }
    }
}

#[bitflags]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionFlag1 {
    /// The byte order used by client for numeric and datetime data types.
    /// (default: little-endian)
    BigEndian = 1 << 0,
    /// The character set used on the client. (default: ASCII)
    CharsetEBDDIC = 1 << 1,
    /// Use VAX floating point representation. (default: IEEE 754)
    FloatVax = 1 << 2,
    /// Use ND5000 floating point representation. (default: IEEE 754)
    FloatND5000 = 1 << 3,
    /// Set is dump/load or BCP capabilities are needed by the client.
    /// (default: ON)
    BcpDumploadOff = 1 << 4,
    /// Set if the client requires warning messages on execution of the USE SQL
    /// statement. If this flag is not set, the server MUST NOT inform the
    /// client when the database changes, and therefore the client will be
    /// unaware of any accompanying collation changes. (default: ON)
    UseDbNotify = 1 << 5,
    /// Set if the change to initial database needs to succeed if the connection
    /// is to succeed. (default: OFF)
    InitDbFatal = 1 << 6,
    /// Set if the client requires warning messages on execution of a language
    /// change statement. (default: OFF)
    LangChangeWarn = 1 << 7,
}

#[bitflags]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionFlag2 {
    /// Set if the change to initial language needs to succeed if the connect is
    /// to succeed.
    InitLangFatal = 1 << 0,
    /// Set if the client is the ODBC driver. This causes the server to set
    /// `ANSI_DEFAULTS=ON`, `CURSOR_CLOSE_ON_COMMIT`, `IMPLICIT_TRANSACTIONS=OFF`,
    /// `TEXTSIZE=0x7FFFFFFF` (2GB) (TDS 7.2 and earlier) `TEXTSIZE` to infinite
    /// (TDS 7.3), and `ROWCOUNT` to infinite.
    OdbcDriver = 1 << 1,
    /// (not documented)
    TransBoundary = 1 << 2,
    /// (not documented)
    CacheConnect = 1 << 3,
    /// Reserved (not really documented)
    UserTypeServer = 1 << 4,
    /// Distributed Query login
    UserTypeRemUser = 1 << 5,
    /// Replication login
    UserTypeSqlRepl = 1 << 6,
    /// Use integrated security in the client.
    IntegratedSecurity = 1 << 7,
}

#[bitflags]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionFlag3 {
    /// Request to change login's password.
    RequestChangePassword = 1 << 0,
    /// XML data type instances are returned as binary XML.
    BinaryXML = 1 << 1,
    /// Client is requesting separate process to be spawned as user instance.
    SpawnUserInstance = 1 << 2,
    /// This bit is used by the server to determine if a client is able to
    /// properly handle collations introduced after TDS 7.2. TDS 7.2 and earlier
    /// clients are encouraged to use this loginpacket bit. Servers MUST ignore
    /// this bit when it is sent by TDS 7.3 or 7.4 clients.
    UnknownCollationHandling = 1 << 3,
    /// ibExtension/cbExtension fields are used.
    ExtensionUsed = 1 << 4,
}

#[bitflags]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginTypeFlag {
    /// Use T-SQL syntax.
    UseTSQL = 1 << 0,
    /// Set if the client is the OLEDB driver. This causes the server to set
    /// ANSI_DEFAULTS to ON, CURSOR_CLOSE_ON_COMMIT and IMPLICIT_TRANSACTIONS to
    /// OFF, TEXTSIZE to 0x7FFFFFFF (2GB) (TDS 7.2 and earlier), TEXTSIZE to
    /// infinite (introduced in TDS 7.3), and ROWCOUNT to infinite.
    UseOLEDB = 1 << 4,
    /// This bit was introduced in TDS 7.4; however, TDS 7.1, 7.2, and 7.3
    /// clients can also use this bit in LOGIN7 to specify that the application
    /// intent of the connection is read-only. The server SHOULD ignore this bit
    /// if the highest TDS version supported by the server is lower than TDS 7.4.
    ReadOnlyIntent = 1 << 5,
}

pub(crate) const FEA_EXT_FEDAUTH: u8 = 0x02u8;
pub(crate) const FEA_EXT_TERMINATOR: u8 = 0xFFu8;
pub(crate) const FED_AUTH_LIBRARYSECURITYTOKEN: u8 = 0x01;

/// https://docs.microsoft.com/en-us/openspecs/windows_protocols/ms-tds/773a62b6-ee89-4c02-9e5e-344882630aac
#[derive(Clone, Default)]
#[cfg_attr(test, derive(PartialEq, Eq))]
struct FedAuthExt<'a> {
    fed_auth_echo: bool,
    fed_auth_token: Cow<'a, str>,
    nonce: Option<[u8; 32]>,
}

impl std::fmt::Debug for FedAuthExt<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FedAuthExt")
            .field("fed_auth_echo", &self.fed_auth_echo)
            .field("fed_auth_token", &"<HIDDEN>")
            .field("nonce", &self.nonce)
            .finish()
    }
}

/// the login packet
#[derive(Clone, Default)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct LoginMessage<'a> {
    /// the highest TDS version the client supports
    tds_version: FeatureLevel,
    /// the requested packet size
    packet_size: u32,
    /// the version of the interface library
    client_prog_ver: u32,
    /// the process id of the client application
    client_pid: u32,
    /// the connection id of the primary server
    /// (used when connecting to an "Always UP" backup server)
    connection_id: u32,
    option_flags_1: BitFlags<OptionFlag1>,
    option_flags_2: BitFlags<OptionFlag2>,
    /// flag included in option_flags_2
    integrated_security: Option<Vec<u8>>,
    type_flags: BitFlags<LoginTypeFlag>,
    option_flags_3: BitFlags<OptionFlag3>,
    client_timezone: i32,
    client_lcid: u32,
    hostname: Cow<'a, str>,
    username: Cow<'a, str>,
    password: Cow<'a, str>,
    app_name: Cow<'a, str>,
    server_name: Cow<'a, str>,
    /// the default database to connect to
    db_name: Cow<'a, str>,
    fed_auth_ext: Option<FedAuthExt<'a>>,
}

// Hand-written so the password and AAD token are never printed.
impl std::fmt::Debug for LoginMessage<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginMessage")
            .field("tds_version", &self.tds_version)
            .field("packet_size", &self.packet_size)
            .field("option_flags_1", &self.option_flags_1)
            .field("option_flags_2", &self.option_flags_2)
            .field("type_flags", &self.type_flags)
            .field("option_flags_3", &self.option_flags_3)
            .field("hostname", &self.hostname)
            .field("username", &self.username)
            .field("password", &"<HIDDEN>")
            .field("app_name", &self.app_name)
            .field("server_name", &self.server_name)
            .field("db_name", &self.db_name)
            .field("fed_auth_ext", &self.fed_auth_ext)
            .finish_non_exhaustive()
    }
}

impl<'a> LoginMessage<'a> {
    pub fn new() -> LoginMessage<'a> {
        Self {
            packet_size: 4096,
            option_flags_1: OptionFlag1::UseDbNotify | OptionFlag1::InitDbFatal,
            option_flags_2: OptionFlag2::InitLangFatal | OptionFlag2::OdbcDriver,
            option_flags_3: BitFlags::from_flag(OptionFlag3::UnknownCollationHandling),
            app_name: "mssql".into(),
            ..Default::default()
        }
    }

    #[cfg(any(
        all(unix, any(feature = "integrated-auth-gssapi", feature = "sspi-rs")),
        windows
    ))]
    pub fn integrated_security(&mut self, bytes: Option<Vec<u8>>) {
        if bytes.is_some() {
            self.option_flags_2.insert(OptionFlag2::IntegratedSecurity);
        } else {
            self.option_flags_2.remove(OptionFlag2::IntegratedSecurity);
        }

        self.integrated_security = bytes;
    }

    pub fn app_name(&mut self, name: impl Into<Cow<'a, str>>) {
        self.app_name = name.into();
    }

    pub fn db_name(&mut self, db_name: impl Into<Cow<'a, str>>) {
        self.db_name = db_name.into();
    }

    pub fn server_name(&mut self, server_name: impl Into<Cow<'a, str>>) {
        self.server_name = server_name.into();
    }

    /// The client machine's hostname, sent in the LOGIN7 packet. Unrelated to
    /// [`server_name`], which names the SQL Server instance being connected
    /// to.
    ///
    /// [`server_name`]: #method.server_name
    pub fn hostname(&mut self, hostname: impl Into<Cow<'a, str>>) {
        self.hostname = hostname.into();
    }

    pub fn user_name(&mut self, user_name: impl Into<Cow<'a, str>>) {
        self.username = user_name.into();
    }

    pub fn password(&mut self, password: impl Into<Cow<'a, str>>) {
        self.password = password.into();
    }

    pub fn aad_token(
        &mut self,
        token: impl Into<Cow<'a, str>>,
        fed_auth_echo: bool,
        nonce: Option<[u8; 32]>,
    ) {
        self.option_flags_3.insert(OptionFlag3::ExtensionUsed);

        self.fed_auth_ext = Some(FedAuthExt {
            fed_auth_echo,
            fed_auth_token: token.into(),
            nonce,
        })
    }

    pub fn readonly(&mut self, readonly: bool) {
        if readonly {
            self.type_flags.insert(LoginTypeFlag::ReadOnlyIntent);
        } else {
            self.type_flags.remove(LoginTypeFlag::ReadOnlyIntent);
        }
    }

    /// Sets the requested TDS packet size. The caller (see
    /// [`crate::Config::packet_size`]) is responsible for validating the
    /// value; this is a thin, unchecked wire-encoding setter.
    pub(crate) fn packet_size(&mut self, size: u32) {
        self.packet_size = size;
    }

    /// An upper bound on the encoded length. Every `&str` field encodes as
    /// UTF-16, which never takes more than 2 bytes per UTF-8 byte; the fixed
    /// part is 94 bytes and the FedAuth FeatureExt adds at most 51 bytes of
    /// framing plus the token. Reserving this up front means the buffer that
    /// holds the (obfuscated) password and the AAD token never reallocates,
    /// so no unzeroized copy of a credential is left behind in freed heap.
    /// Same goal as upstream Tiberius 35f2fe6/d290aea, which compute the exact
    /// length instead.
    fn encoded_len_upper_bound(&self) -> crate::Result<usize> {
        const FIXED_PART: usize = 94;
        const FEATURE_EXT_FRAMING: usize = 64;
        let too_large = login_too_large;

        let strings = [
            &self.hostname,
            &self.username,
            &self.password,
            &self.app_name,
            &self.server_name,
            &self.db_name,
        ]
        .iter()
        .try_fold(0_usize, |total, s| {
            s.len()
                .checked_mul(2)
                .and_then(|bytes| total.checked_add(bytes))
        })
        .ok_or_else(too_large)?;

        let sspi = self.integrated_security.as_ref().map_or(0, Vec::len);

        let fed_auth = match self.fed_auth_ext.as_ref() {
            Some(ext) => ext
                .fed_auth_token
                .len()
                .checked_mul(2)
                .and_then(|token| FEATURE_EXT_FRAMING.checked_add(token))
                .ok_or_else(too_large)?,
            None => 0,
        };

        FIXED_PART
            .checked_add(strings)
            .and_then(|total| total.checked_add(sspi))
            .and_then(|total| total.checked_add(fed_auth))
            .ok_or_else(too_large)
    }

    pub(crate) fn encode_to_vec(self) -> crate::Result<Zeroizing<Vec<u8>>> {
        let reserved = self.encoded_len_upper_bound()?;
        let mut cursor = Cursor::new(Vec::with_capacity(reserved));
        let reserved = cursor.get_ref().capacity();

        // Space for the length
        cursor.write_u32::<LittleEndian>(0)?;

        cursor.write_u32::<LittleEndian>(u32::from(self.tds_version))?;
        cursor.write_u32::<LittleEndian>(self.packet_size)?;
        cursor.write_u32::<LittleEndian>(self.client_prog_ver)?;
        cursor.write_u32::<LittleEndian>(self.client_pid)?;
        cursor.write_u32::<LittleEndian>(self.connection_id)?;

        cursor.write_u8(self.option_flags_1.bits())?;
        cursor.write_u8(self.option_flags_2.bits())?;
        cursor.write_u8(self.type_flags.bits())?;
        cursor.write_u8(self.option_flags_3.bits())?;

        cursor.write_i32::<LittleEndian>(self.client_timezone)?;
        cursor.write_u32::<LittleEndian>(self.client_lcid)?;

        // variable length data (OffsetLength)
        let var_data = [
            &self.hostname,
            &self.username,
            &self.password,
            &self.app_name,
            &self.server_name,
            &"".into(), // 5. ibExtension
            &"".into(), // ibCltIntName
            &"".into(), // ibLanguage
            &self.db_name,
            &"".into(), // 9. ClientId (6 bytes); this is included in var_data so we don't lack the bytes of cbSspiLong (4=2*2) and can insert it at the correct position
            &"".into(), // 10. ibSSPI
            &"".into(), // ibAtchDBFile
            &"".into(), // ibChangePassword
        ];

        // The fixed OffsetLength table: 13 entries of (offset, length) u16
        // pairs, plus the 6-byte ClientId, follow the current position.
        let mut data_offset = convert::<_, usize>(cursor.position(), "login_offset")?
            .checked_add(
                var_data
                    .len()
                    .checked_mul(2 * 2)
                    .and_then(|table| table.checked_add(6))
                    .ok_or_else(login_too_large)?,
            )
            .ok_or_else(login_too_large)?;
        let mut fea_ext_offset = 0;

        for (i, value) in var_data.iter().enumerate() {
            if i == 5 {
                // we might need to update the feature ext potion later
                fea_ext_offset = cursor.position();
            }

            // write the client ID (created from the MAC address)
            if i == 9 {
                cursor.write_u32::<LittleEndian>(0)?; //TODO:
                cursor.write_u16::<LittleEndian>(42)?; //TODO: generate real client id
                continue;
            }

            cursor.write_u16::<LittleEndian>(convert(data_offset, "login_offset")?)?;

            // ibSSPI
            if i == 10 {
                let length = if let Some(ref bytes) = self.integrated_security {
                    let bak = cursor.position();

                    cursor.set_position(convert(data_offset, "login_offset")?);
                    cursor.write_all(bytes)?;

                    data_offset = data_offset
                        .checked_add(bytes.len())
                        .ok_or_else(login_too_large)?;
                    cursor.set_position(bak);

                    bytes.len()
                } else {
                    0
                };

                cursor.write_u16::<LittleEndian>(convert(length, "login_sspi_length")?)?;

                continue;
            }

            // jump into the data portion of the output
            let bak = cursor.position();
            cursor.set_position(convert(data_offset, "login_offset")?);

            for codepoint in value.encode_utf16() {
                cursor.write_u16::<LittleEndian>(codepoint)?;
            }

            let new_position = convert::<_, usize>(cursor.position(), "login_offset")?;

            // prepare the password in MS-fashion
            if i == 2 {
                let buffer = cursor.get_mut();
                for byte in buffer.iter_mut().take(new_position).skip(data_offset) {
                    *byte = ((*byte << 4) & 0xf0 | (*byte >> 4) & 0x0f) ^ 0xA5;
                }
            }

            let length = new_position
                .checked_sub(data_offset)
                .ok_or_else(login_too_large)?;
            cursor.set_position(bak);
            data_offset = data_offset
                .checked_add(length)
                .ok_or_else(login_too_large)?;

            // microsoft being really consistent here... using byte offsets with utf16-length's
            // sounds like premature optimization
            // (`length` is a whole number of UTF-16 code units, so it is even.)
            let units = length.checked_div(2).ok_or_else(login_too_large)?;
            cursor.write_u16::<LittleEndian>(convert(units, "login_field_length")?)?;
        }

        // cbSSPILong
        cursor.write_u32::<LittleEndian>(0)?;

        // FeatureExt
        if let Some(fed_auth_ext) = self.fed_auth_ext {
            // update fea_ext_offset
            cursor.set_position(fea_ext_offset);
            cursor.write_u16::<LittleEndian>(convert(data_offset, "login_offset")?)?;
            cursor.write_u16::<LittleEndian>(4)?;

            cursor.set_position(convert(data_offset, "login_offset")?);
            data_offset = data_offset.checked_add(4).ok_or_else(login_too_large)?;
            cursor.write_u32::<LittleEndian>(convert(data_offset, "login_offset")?)?;

            cursor.write_u8(FEA_EXT_FEDAUTH)?;

            // Pre-sized for the same reason as the outer buffer: growing it
            // would leave copies of the bearer token in freed memory.
            let mut token =
                Zeroizing::new(Vec::with_capacity(fed_auth_ext.fed_auth_token.len() * 2));
            for codepoint in fed_auth_ext.fed_auth_token.encode_utf16() {
                token.extend_from_slice(&codepoint.to_le_bytes());
            }

            // options (1) + TokenLength(4) + Token.length + nonce.length
            let nonce_len = if fed_auth_ext.nonce.is_some() { 32 } else { 0 };
            let feature_ext_length = token
                .len()
                .checked_add(1 + 4)
                .and_then(|len| len.checked_add(nonce_len))
                .ok_or_else(login_too_large)?;

            cursor.write_u32::<LittleEndian>(convert(
                feature_ext_length,
                "login_feature_ext_length",
            )?)?;

            let mut options: u8 = FED_AUTH_LIBRARYSECURITYTOKEN << 1;
            if fed_auth_ext.fed_auth_echo {
                options |= 1 // fFedAuthEcho
            }

            cursor.write_u8(options)?;

            cursor.write_u32::<LittleEndian>(convert(token.len(), "login_token_length")?)?;
            cursor.write_all(token.as_slice())?;
            token.zeroize();

            if let Some(nonce) = fed_auth_ext.nonce {
                cursor.write_all(nonce.as_ref())?;
            }

            cursor.write_u8(FEA_EXT_TERMINATOR)?;
        }

        cursor.set_position(0);
        let total_length = convert(cursor.get_ref().len(), "login_length")?;
        cursor.write_u32::<LittleEndian>(total_length)?;

        debug_assert_eq!(
            reserved,
            cursor.get_ref().capacity(),
            "LOGIN7 buffer reallocated; encoded_len_upper_bound is too small"
        );

        Ok(Zeroizing::new(cursor.into_inner()))
    }
}

impl<'a> Encode<BytesMut> for LoginMessage<'a> {
    fn encode(self, dst: &mut BytesMut) -> crate::Result<()> {
        let mut encoded = self.encode_to_vec()?;
        dst.extend_from_slice(encoded.as_slice());
        encoded.zeroize();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn oversized_fields_are_a_conversion_error_not_a_truncated_offset() {
        // 13 offset/length pairs + fixed header take 75 bytes, and every
        // UTF-16 code unit takes 2: 40_000 characters push the next offset
        // past what a u16 offset field can hold (65_535).
        let mut login = LoginMessage::new();
        login.user_name("u".repeat(40_000));
        login.password("p");
        let err = login.encode_to_vec().unwrap_err();
        assert_eq!(
            err,
            crate::Error::Conversion("login_offset: value does not fit the target type".into())
        );
    }

    #[test]
    fn a_login_that_fits_still_encodes_with_its_exact_length() {
        let mut login = LoginMessage::new();
        login.user_name("user");
        login.password("pw");
        let wire = login.encode_to_vec().unwrap();
        let declared = u32::from_le_bytes([wire[0], wire[1], wire[2], wire[3]]);
        assert_eq!(usize::try_from(declared).unwrap(), wire.len());
    }

    #[test]
    fn upper_bound_covers_the_encoded_length() {
        let mut login = LoginMessage::new();
        login.user_name("user");
        let bound = login.encoded_len_upper_bound().unwrap();
        let wire = login.encode_to_vec().unwrap();
        assert!(bound >= wire.len());
    }

    #[test]
    fn feature_level_discriminants_and_ordering() {
        assert_eq!(u32::from(FeatureLevel::SqlServer2008), 0x730A_0003);
        assert!(FeatureLevel::SqlServer2005 < FeatureLevel::SqlServer2008);
        assert_eq!(FeatureLevel::SqlServer2005.done_row_count_bytes(), 8);
        assert_eq!(FeatureLevel::SqlServer2000.done_row_count_bytes(), 4);
        assert_eq!(
            FeatureLevel::try_from(0x7400_0004_u32),
            Ok(FeatureLevel::SqlServerN)
        );
        assert_eq!(FeatureLevel::try_from(0x1234_u32), Err(()));
    }

    #[test]
    fn uint_enum_u8_conversion_matches_the_low_byte_of_the_discriminant() {
        // `TryFrom<u8>` compares against the discriminant's low byte (the
        // truncation the macro has always documented).
        assert_eq!(
            FeatureLevel::try_from(0x00_u8),
            Ok(FeatureLevel::SqlServerV7)
        );
        assert_eq!(
            FeatureLevel::try_from(0x04_u8),
            Ok(FeatureLevel::SqlServerN)
        );
        assert_eq!(FeatureLevel::try_from(0x05_u8), Err(()));
    }

    use super::*;
    use crate::Decode;
    use byteorder::ReadBytesExt;
    use bytes::BytesMut;
    use std::io::Read;

    impl<'a> Decode<BytesMut> for LoginMessage<'a> {
        fn decode(src: &mut BytesMut) -> crate::Result<Self>
        where
            Self: Sized,
        {
            let mut cursor = Cursor::new(src);
            let mut ret = LoginMessage::new();

            let total_length = cursor.read_u32::<LittleEndian>()?;

            ret.tds_version = cursor
                .read_u32::<LittleEndian>()?
                .try_into()
                .expect("tds_version verification");
            ret.packet_size = cursor.read_u32::<LittleEndian>()?;
            ret.client_prog_ver = cursor.read_u32::<LittleEndian>()?;
            ret.client_pid = cursor.read_u32::<LittleEndian>()?;
            ret.connection_id = cursor.read_u32::<LittleEndian>()?;

            ret.option_flags_1 =
                BitFlags::from_bits(cursor.read_u8()?).expect("option_flags_1 verification");
            ret.option_flags_2 =
                BitFlags::from_bits(cursor.read_u8()?).expect("option_flags_2 verification");
            ret.type_flags =
                BitFlags::from_bits(cursor.read_u8()?).expect("type_flags verification");
            ret.option_flags_3 =
                BitFlags::from_bits(cursor.read_u8()?).expect("option_flags_3 verification");

            ret.client_timezone = cursor.read_u32::<LittleEndian>()? as i32;
            ret.client_lcid = cursor.read_u32::<LittleEndian>()?;

            macro_rules! read_offset_length_bytes {
                () => {{
                    let offset = cursor.read_u16::<LittleEndian>()?;
                    let length = cursor.read_u16::<LittleEndian>()?;
                    let pos = cursor.position();
                    cursor.set_position(offset as u64);

                    let mut values = vec![0u8; length as usize];
                    cursor.read_exact(&mut values)?;

                    cursor.set_position(pos);
                    values
                }};
            }

            macro_rules! read_offset_length_string {
                () => {
                    read_offset_length_string!("")
                };
                ($tag:expr) => {{
                    let offset = cursor.read_u16::<LittleEndian>()?;
                    let length = cursor.read_u16::<LittleEndian>()?;
                    let pos = cursor.position();
                    cursor.set_position(offset as u64);

                    if $tag == "password" {
                        let buffer = cursor.get_mut();
                        for byte in buffer
                            .iter_mut()
                            .skip(offset as usize)
                            .take(length as usize * 2)
                        {
                            *byte ^= 0xA5;
                            *byte = ((*byte << 4) & 0xf0 | (*byte >> 4) & 0x0f);
                        }
                    }

                    let mut values = vec![0u16; length as usize];
                    cursor.read_u16_into::<LittleEndian>(&mut values)?;
                    cursor.set_position(pos);

                    String::from_utf16(&values).expect("decode utf16")
                }};
            }

            ret.hostname = read_offset_length_string!().into();
            ret.username = read_offset_length_string!().into();
            ret.password = read_offset_length_string!("password").into();
            ret.app_name = read_offset_length_string!().into();
            ret.server_name = read_offset_length_string!().into();
            let fea_ext_offset = read_offset_length_bytes!(); // 5. ibExtension
            let fea_ext_offset = if fea_ext_offset.len() == 4 {
                u32::from_le_bytes(fea_ext_offset.try_into().unwrap())
            } else {
                0
            };
            let _ = read_offset_length_string!(); // ibCltIntName
            let _ = read_offset_length_string!(); // ibLanguage
            ret.db_name = read_offset_length_string!().into();
            // 9. ClientId (6 bytes); this is included in var_data so we don't lack the bytes of cbSspiLong (4=2*2) and can insert it at the correct position
            let _ = cursor.read_u32::<LittleEndian>()?;
            let _ = cursor.read_u16::<LittleEndian>()?;
            let is = read_offset_length_bytes!();
            ret.integrated_security = if is.is_empty() { None } else { Some(is) };
            let _ = read_offset_length_string!(); // ibAtchDBFile
            let _ = read_offset_length_string!(); // ibChangePassword
                                                  // let _ = cursor.read_u32::<LittleEndian>()?;
                                                  // cbSSPILong

            if fea_ext_offset != 0 {
                cursor.set_position((fea_ext_offset) as u64);

                assert!(ret.option_flags_3.contains(OptionFlag3::ExtensionUsed));
                loop {
                    let fe = cursor.read_u8()?;
                    if fe == FEA_EXT_TERMINATOR {
                        break;
                    } else if fe == FEA_EXT_FEDAUTH {
                        let fea_ext_len = cursor.read_u32::<LittleEndian>()?;
                        let pos = cursor.position();
                        let mut options = cursor.read_u8()?;
                        let fed_auth_echo = (options & 1) == 1;
                        options >>= 1;
                        if options != FED_AUTH_LIBRARYSECURITYTOKEN {
                            unimplemented!("unsupported FedAuthLibrary {:?}", options);
                        }
                        let token_len = cursor.read_u32::<LittleEndian>()? as usize;
                        let mut token = vec![0u16; token_len / 2];
                        cursor.read_u16_into::<LittleEndian>(&mut token)?;
                        let token = String::from_utf16(&token).expect("decode utf16");
                        let remaining = fea_ext_len - (cursor.position() - pos) as u32;
                        let nonce = if remaining == 32 {
                            let mut a = [0u8; 32];
                            cursor.read_exact(&mut a)?;
                            Some(a)
                        } else if remaining == 0 {
                            None
                        } else {
                            panic!("read feature ext fail: {}", remaining);
                        };

                        let fed_auth_ext = FedAuthExt {
                            fed_auth_echo,
                            fed_auth_token: token.into(),
                            nonce,
                        };
                        ret.fed_auth_ext = Some(fed_auth_ext);
                    } else {
                        unimplemented!("unsupported feature ext {:?}", fe);
                    }
                }
            }

            assert!(cursor.position() <= total_length as u64);

            Ok(ret)
        }
    }

    #[test]
    fn login_message_round_trip() {
        let mut payload = BytesMut::new();
        let mut login = LoginMessage::new();
        login.db_name("fake-database-name");
        login.app_name("fake-app-name");
        login.server_name("fake-server-name");
        login.hostname("fake-client-hostname");
        login.user_name("fake-user-name");
        login.password("fake-pw");
        login
            .clone()
            .encode(&mut payload)
            .expect("encode should succeed");

        let decoded = LoginMessage::decode(&mut payload).expect("decode should succeed");

        assert_eq!(login, decoded);
    }

    #[test]
    fn specify_aad_token() {
        let mut login = LoginMessage::new();
        let token = "fake-aad-token";
        let nonce = [3u8; 32];
        login.aad_token(token, true, Some(nonce));

        assert!(login.option_flags_3.contains(OptionFlag3::ExtensionUsed));
        assert_eq!(
            login.fed_auth_ext.expect("fed_auto_specified"),
            FedAuthExt {
                fed_auth_echo: true,
                fed_auth_token: token.into(),
                nonce: Some(nonce)
            }
        )
    }

    #[test]
    fn login_message_with_fed_auth_round_trip() {
        let mut payload = BytesMut::new();
        let mut login = LoginMessage::new();
        let nonce = [1u8; 32];
        login.aad_token("fake-aad-token", true, Some(nonce));
        login
            .clone()
            .encode(&mut payload)
            .expect("encode should succeed");

        let decoded = LoginMessage::decode(&mut payload).expect("decode should succeed");

        assert_eq!(login, decoded);
    }

    // The debug_assert in `encode_to_vec` fails if the buffer holding the
    // credentials ever reallocates; exercise it with fields far larger than
    // the old fixed 512-byte reservation, including multi-byte characters.
    #[test]
    fn large_credentials_do_not_reallocate_the_login_buffer() {
        let big = "pässwörd-€-".repeat(200);
        let mut login = LoginMessage::new();
        login.user_name("user-".repeat(100));
        login.password(big.clone());
        login.db_name("db-".repeat(100));
        let encoded = login.clone().encode_to_vec().unwrap();
        assert!(encoded.len() > 4096);

        let mut aad = LoginMessage::new();
        aad.aad_token("t".repeat(20_000), true, Some([7u8; 32]));
        let encoded = aad.encode_to_vec().unwrap();
        assert!(encoded.len() > 40_000);
    }

    #[test]
    fn login_debug_hides_password_and_token() {
        let mut login = LoginMessage::new();
        login.password("hunter2-secret");
        login.aad_token("aad-secret-token", true, None);
        let shown = format!("{login:?}");
        assert!(!shown.contains("hunter2"), "{shown}");
        assert!(!shown.contains("aad-secret"), "{shown}");
    }

    #[test]
    fn integrated_security_and_read_only_flags_can_be_enabled_and_cleared() {
        let mut login = LoginMessage::new();

        #[cfg(any(
            all(unix, any(feature = "integrated-auth-gssapi", feature = "sspi-rs")),
            windows
        ))]
        {
            login.integrated_security(Some(vec![1, 2, 3]));
            assert!(login
                .option_flags_2
                .contains(OptionFlag2::IntegratedSecurity));
            assert_eq!(login.integrated_security.as_deref(), Some(&[1, 2, 3][..]));

            login.integrated_security(None);
            assert!(!login
                .option_flags_2
                .contains(OptionFlag2::IntegratedSecurity));
            assert!(login.integrated_security.is_none());
        }

        login.readonly(true);
        assert!(login.type_flags.contains(LoginTypeFlag::ReadOnlyIntent));
        login.readonly(false);
        assert!(!login.type_flags.contains(LoginTypeFlag::ReadOnlyIntent));
    }

    #[test]
    fn a_federated_authentication_token_is_encoded_as_utf16() {
        let mut login = LoginMessage::new();
        login.user_name("user");
        login.aad_token("tok", true, Some([3; 32]));
        let bound = login.encoded_len_upper_bound().unwrap();
        let wire = login.encode_to_vec().unwrap();
        assert!(bound >= wire.len());
        let token: Vec<u8> = "tok".encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert!(wire.windows(token.len()).any(|w| w == token.as_slice()));
    }

    #[cfg(any(
        all(unix, any(feature = "integrated-auth-gssapi", feature = "sspi-rs")),
        windows
    ))]
    #[test]
    fn an_sspi_token_is_carried_in_the_login_data() {
        let token = vec![0xa1, 0xb2, 0xc3, 0xd4, 0xe5];
        let mut login = LoginMessage::new();
        login.user_name("user");
        login.integrated_security(Some(token.clone()));
        let bound = login.encoded_len_upper_bound().unwrap();
        let with_token = login.encode_to_vec().unwrap();
        assert!(bound >= with_token.len());
        assert!(with_token
            .windows(token.len())
            .any(|window| window == token.as_slice()));

        // Clearing the token removes it again.
        let mut login = LoginMessage::new();
        login.user_name("user");
        login.integrated_security(Some(token));
        login.integrated_security(None);
        assert!(login.encode_to_vec().unwrap().len() < with_token.len());
    }
}
