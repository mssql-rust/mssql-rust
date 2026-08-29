mod ado_net;
mod jdbc;

use std::collections::HashMap;
use std::path::PathBuf;

use super::AuthMethod;
use crate::EncryptionLevel;
use ado_net::*;
use jdbc::*;

#[derive(Clone, Debug)]
/// The `Config` struct contains all configuration information
/// required for connecting to the database with a [`Client`]. It also provides
/// the server address when connecting to a `TcpStream` via the
/// [`get_addr`] method.
///
/// When using an [ADO.NET connection string], it can be
/// constructed using the [`from_ado_string`] function.
///
/// [`Client`]: struct.Client.html
/// [ADO.NET connection string]: https://docs.microsoft.com/en-us/dotnet/framework/data/adonet/connection-strings
/// [`from_ado_string`]: struct.Config.html#method.from_ado_string
/// [`get_addr`]: struct.Config.html#method.get_addr
pub struct Config {
    pub(crate) host: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) database: Option<String>,
    pub(crate) instance_name: Option<String>,
    pub(crate) application_name: Option<String>,
    pub(crate) client_name: Option<String>,
    pub(crate) encryption: EncryptionLevel,
    pub(crate) trust: TrustConfig,
    pub(crate) host_name_in_certificate: Option<String>,
    pub(crate) auth: AuthMethod,
    pub(crate) readonly: bool,
    pub(crate) send_string_parameters_as_unicode: bool,
    pub(crate) multi_subnet_failover: bool,
    pub(crate) packet_size: Option<u32>,
}

/// The valid range for [`Config::packet_size`], per the TDS `LOGIN7`
/// packet's `PacketSize` field.
const PACKET_SIZE_RANGE: std::ops::RangeInclusive<u32> = 512..=32767;

#[derive(Clone, Debug)]
pub(crate) enum TrustConfig {
    #[allow(dead_code)]
    CaCertificateLocation(PathBuf),
    TrustAll,
    Default,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: None,
            port: None,
            database: None,
            instance_name: None,
            application_name: None,
            client_name: None,
            #[cfg(any(
                feature = "rustls",
                feature = "native-tls",
                feature = "vendored-openssl"
            ))]
            encryption: EncryptionLevel::Required,
            #[cfg(not(any(
                feature = "rustls",
                feature = "native-tls",
                feature = "vendored-openssl"
            )))]
            encryption: EncryptionLevel::NotSupported,
            trust: TrustConfig::Default,
            host_name_in_certificate: None,
            auth: AuthMethod::None,
            readonly: false,
            send_string_parameters_as_unicode: true,
            multi_subnet_failover: false,
            packet_size: None,
        }
    }
}

impl Config {
    /// Create a new `Config` with the default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new [`ConfigBuilder`], initialized with the default
    /// settings, offering the same options as `Config`'s own setters but
    /// through a chainable, `.build()`-terminated API. Equivalent to and
    /// interchangeable with calling `Config::new()` and its setters
    /// directly - use whichever style reads better for the call site.
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder {
            inner: Self::default(),
        }
    }

    /// A host or ip address to connect to.
    ///
    /// - Defaults to `localhost`.
    pub fn host(&mut self, host: impl ToString) {
        self.host = Some(host.to_string());
    }

    /// The server port.
    ///
    /// - Defaults to `1433`.
    pub fn port(&mut self, port: u16) {
        self.port = Some(port);
    }

    /// The database to connect to.
    ///
    /// - Defaults to `master`.
    pub fn database(&mut self, database: impl ToString) {
        self.database = Some(database.to_string())
    }

    /// The instance name as defined in the SQL Browser. Only available on
    /// Windows platforms.
    ///
    /// If specified, the port is replaced with the value returned from the
    /// browser.
    ///
    /// - Defaults to no name specified.
    pub fn instance_name(&mut self, name: impl ToString) {
        self.instance_name = Some(name.to_string());
    }

    /// Sets the application name to the connection, queryable with the
    /// `APP_NAME()` command.
    ///
    /// - Defaults to no name specified.
    pub fn application_name(&mut self, name: impl ToString) {
        self.application_name = Some(name.to_string());
    }

    /// Sets the client machine's hostname, sent in the LOGIN7 packet as the
    /// workstation name. Unrelated to [`host`], which is the address of the
    /// SQL Server to connect to.
    ///
    /// - Defaults to no name specified.
    ///
    /// [`host`]: #method.host
    pub fn client_name(&mut self, name: impl ToString) {
        self.client_name = Some(name.to_string());
    }

    /// Set the preferred encryption level.
    ///
    /// - With `tls` feature, defaults to `Required`.
    /// - Without `tls` feature, defaults to `NotSupported`.
    pub fn encryption(&mut self, encryption: EncryptionLevel) {
        self.encryption = encryption;
    }

    /// If set, the server certificate will not be validated and it is accepted
    /// as-is.
    ///
    /// On production setting, the certificate should be added to the local key
    /// storage (or use `trust_cert_ca` instead), using this setting is potentially dangerous.
    ///
    /// # Panics
    /// Will panic in case `trust_cert_ca` was called before.
    ///
    /// - Defaults to `default`, meaning server certificate is validated against system-truststore.
    pub fn trust_cert(&mut self) {
        if let TrustConfig::CaCertificateLocation(_) = &self.trust {
            panic!("'trust_cert' and 'trust_cert_ca' are mutual exclusive! Only use one.")
        }
        self.trust = TrustConfig::TrustAll;
    }

    /// If set, the server certificate will be validated against the given CA certificate in
    /// in addition to the system-truststore.
    /// Useful when using self-signed certificates on the server without having to disable the
    /// trust-chain.
    ///
    /// # Panics
    /// Will panic in case `trust_cert` was called before.
    ///
    /// - Defaults to validating the server certificate is validated against system's certificate storage.
    pub fn trust_cert_ca(&mut self, path: impl ToString) {
        if let TrustConfig::TrustAll = &self.trust {
            panic!("'trust_cert' and 'trust_cert_ca' are mutual exclusive! Only use one.")
        } else {
            self.trust = TrustConfig::CaCertificateLocation(PathBuf::from(path.to_string()))
        }
    }

    /// Overrides the hostname used to validate the server's TLS certificate,
    /// independent of the address given to [`host`]. Useful when connecting
    /// through a proxy, load balancer, or an address that doesn't match the
    /// name on the certificate (mirrors the JDBC driver's
    /// `hostNameInCertificate` property).
    ///
    /// - Defaults to using the address from [`host`].
    ///
    /// [`host`]: #method.host
    pub fn host_name_in_certificate(&mut self, name: impl ToString) {
        self.host_name_in_certificate = Some(name.to_string());
    }

    /// Sets the authentication method.
    ///
    /// - Defaults to `None`.
    pub fn authentication(&mut self, auth: AuthMethod) {
        self.auth = auth;
    }

    /// Sets ApplicationIntent readonly.
    ///
    /// - Defaults to `false`.
    pub fn readonly(&mut self, readnoly: bool) {
        self.readonly = readnoly;
    }

    /// Controls whether `&str` and `String` query parameters are sent to the
    /// server as `NVARCHAR` (Unicode).
    ///
    /// SQL Server's default behavior of always treating string parameters as
    /// `NVARCHAR` can defeat an index on a `VARCHAR` column, since comparing
    /// a `VARCHAR` column against an `NVARCHAR` literal requires an implicit
    /// conversion of every row. Setting this to `false` sends string
    /// parameters as `VARCHAR` instead (mirrors the JDBC/ODBC drivers'
    /// `sendStringParametersAsUnicode` connection property).
    ///
    /// - Defaults to `true`.
    pub fn send_string_parameters_as_unicode(&mut self, enabled: bool) {
        self.send_string_parameters_as_unicode = enabled;
    }

    /// Sets the `MultiSubnetFailover` flag, hinting that the target is a SQL
    /// Server Always On availability group listener. When enabled, all of
    /// the listener's resolved IP addresses are connected to in parallel
    /// (rather than sequentially, one at a time) and the first to succeed
    /// wins, substantially reducing failover/reconnect time when the
    /// addresses span multiple subnets.
    ///
    /// - Defaults to `false`.
    pub fn multi_subnet_failover(&mut self, multi_subnet_failover: bool) {
        self.multi_subnet_failover = multi_subnet_failover;
    }

    /// Gets the `MultiSubnetFailover` flag.
    pub fn get_multi_subnet_failover(&self) -> bool {
        self.multi_subnet_failover
    }

    /// Sets the requested TDS packet size for the connection, in bytes.
    ///
    /// Larger packet sizes can reduce the number of network round-trips
    /// needed for large queries or bulk inserts. The server may negotiate a
    /// different size than requested; the actual, negotiated size takes
    /// effect regardless of this setting.
    ///
    /// - Valid range is 512 to 32767 (per the TDS `LOGIN7` packet's
    ///   `PacketSize` field); returns [`Error::Conversion`](crate::Error::Conversion)
    ///   for a value outside that range.
    /// - Defaults to not sending a preference, in which case the server's
    ///   own default (commonly 4096) applies.
    pub fn packet_size(&mut self, size: u32) -> crate::Result<()> {
        if !PACKET_SIZE_RANGE.contains(&size) {
            return Err(crate::Error::Conversion(
                format!(
                    "packet_size must be between {} and {}, got {size}",
                    PACKET_SIZE_RANGE.start(),
                    PACKET_SIZE_RANGE.end(),
                )
                .into(),
            ));
        }

        self.packet_size = Some(size);

        Ok(())
    }

    /// Gets the configured packet size preference, if one was set via
    /// [`Config::packet_size`].
    pub fn get_packet_size(&self) -> Option<u32> {
        self.packet_size
    }

    pub(crate) fn get_host(&self) -> &str {
        self.host
            .as_deref()
            .filter(|v| v != &".")
            .unwrap_or("localhost")
    }

    pub(crate) fn get_port(&self) -> u16 {
        match (self.port, self.instance_name.as_ref()) {
            // A user-defined port, we must use that.
            (Some(port), _) => port,
            // If using a named instance, we'll give the default port of SQL
            // Browser.
            (None, Some(_)) => 1434,
            // Otherwise the defaulting to the default SQL Server port.
            (None, None) => 1433,
        }
    }

    /// Get the host address including port
    pub fn get_addr(&self) -> String {
        format!("{}:{}", self.get_host(), self.get_port())
    }

    /// Creates a new `Config` from an [ADO.NET connection string].
    ///
    /// # Supported parameters
    ///
    /// All parameter keys are handled case-insensitive.
    ///
    /// |Parameter|Allowed values|Description|
    /// |--------|--------|--------|
    /// |`server`|`<string>`|The name or network address of the instance of SQL Server to which to connect. The port number can be specified after the server name. The correct form of this parameter is either `tcp:host,port` or `tcp:host\\instance`|
    /// |`IntegratedSecurity`|`true`,`false`,`yes`,`no`|Toggle between Windows/Kerberos authentication and SQL authentication.|
    /// |`uid`,`username`,`user`,`user id`|`<string>`|The SQL Server login account.|
    /// |`password`,`pwd`|`<string>`|The password for the SQL Server account logging on.|
    /// |`database`|`<string>`|The name of the database.|
    /// |`TrustServerCertificate`|`true`,`false`,`yes`,`no`|Specifies whether the driver trusts the server certificate when connecting using TLS. Cannot be used toghether with `TrustServerCertificateCA`|
    /// |`TrustServerCertificateCA`|`<path>`|Path to a `pem`, `crt` or `der` certificate file. Cannot be used together with `TrustServerCertificate`|
    /// |`encrypt`|`true`,`false`,`yes`,`no`,`DANGER_PLAINTEXT`|Specifies whether the driver uses TLS to encrypt communication.|
    /// |`Application Name`, `ApplicationName`|`<string>`|Sets the application name for the connection.|
    ///
    /// [ADO.NET connection string]: https://docs.microsoft.com/en-us/dotnet/framework/data/adonet/connection-strings
    pub fn from_ado_string(s: &str) -> crate::Result<Self> {
        let ado: AdoNetConfig = s.parse()?;
        Self::from_config_string(ado)
    }

    /// Creates a new `Config` from a [JDBC connection string].
    ///
    /// See [`from_ado_string`] method for supported parameters.
    ///
    /// [JDBC connection string]: https://docs.microsoft.com/en-us/sql/connect/jdbc/building-the-connection-url?view=sql-server-ver15
    /// [`from_ado_string`]: #method.from_ado_string
    pub fn from_jdbc_string(s: &str) -> crate::Result<Self> {
        let jdbc: JdbcConfig = s.parse()?;
        Self::from_config_string(jdbc)
    }

    fn from_config_string(s: impl ConfigString) -> crate::Result<Self> {
        let mut builder = Self::new();

        let server = s.server()?;

        if let Some(host) = server.host {
            builder.host(host);
        }

        if let Some(port) = server.port {
            builder.port(port);
        }

        if let Some(instance) = server.instance {
            builder.instance_name(instance);
        }

        builder.authentication(s.authentication()?);

        if let Some(database) = s.database() {
            builder.database(database);
        }

        if let Some(name) = s.application_name() {
            builder.application_name(name);
        }

        if s.trust_cert()? {
            builder.trust_cert();
        }

        if let Some(ca) = s.trust_cert_ca() {
            builder.trust_cert_ca(ca);
        }

        builder.encryption(s.encrypt()?);

        builder.readonly(s.readonly());

        builder.multi_subnet_failover(s.multi_subnet_failover()?);

        Ok(builder)
    }
}

/// A chainable builder for [`Config`], created via [`Config::builder`].
///
/// This is a pure alternative to constructing a `Config` with `Config::new()`
/// and calling its setters directly - both styles produce the same `Config`
/// and remain fully supported; use whichever reads better at the call site.
///
/// # Example
///
/// ```
/// # use mssql::{Config, AuthMethod};
/// let config = Config::builder()
///     .host("localhost")
///     .port(1433)
///     .authentication(AuthMethod::sql_server("SA", "<YourStrong@Passw0rd>"))
///     .trust_cert()
///     .build();
/// ```
#[derive(Clone, Debug)]
pub struct ConfigBuilder {
    inner: Config,
}

impl ConfigBuilder {
    /// See [`Config::host`].
    pub fn host(&mut self, host: impl ToString) -> &mut Self {
        self.inner.host(host);
        self
    }

    /// See [`Config::port`].
    pub fn port(&mut self, port: u16) -> &mut Self {
        self.inner.port(port);
        self
    }

    /// See [`Config::database`].
    pub fn database(&mut self, database: impl ToString) -> &mut Self {
        self.inner.database(database);
        self
    }

    /// See [`Config::instance_name`].
    pub fn instance_name(&mut self, name: impl ToString) -> &mut Self {
        self.inner.instance_name(name);
        self
    }

    /// See [`Config::application_name`].
    pub fn application_name(&mut self, name: impl ToString) -> &mut Self {
        self.inner.application_name(name);
        self
    }

    /// See [`Config::client_name`].
    pub fn client_name(&mut self, name: impl ToString) -> &mut Self {
        self.inner.client_name(name);
        self
    }

    /// See [`Config::encryption`].
    pub fn encryption(&mut self, encryption: EncryptionLevel) -> &mut Self {
        self.inner.encryption(encryption);
        self
    }

    /// See [`Config::trust_cert`].
    ///
    /// # Panics
    /// Will panic in case `trust_cert_ca` was called before.
    pub fn trust_cert(&mut self) -> &mut Self {
        self.inner.trust_cert();
        self
    }

    /// See [`Config::trust_cert_ca`].
    ///
    /// # Panics
    /// Will panic in case `trust_cert` was called before.
    pub fn trust_cert_ca(&mut self, path: impl ToString) -> &mut Self {
        self.inner.trust_cert_ca(path);
        self
    }

    /// See [`Config::host_name_in_certificate`].
    pub fn host_name_in_certificate(&mut self, name: impl ToString) -> &mut Self {
        self.inner.host_name_in_certificate(name);
        self
    }

    /// See [`Config::authentication`].
    pub fn authentication(&mut self, auth: AuthMethod) -> &mut Self {
        self.inner.authentication(auth);
        self
    }

    /// See [`Config::readonly`].
    pub fn readonly(&mut self, readonly: bool) -> &mut Self {
        self.inner.readonly(readonly);
        self
    }

    /// See [`Config::send_string_parameters_as_unicode`].
    pub fn send_string_parameters_as_unicode(&mut self, enabled: bool) -> &mut Self {
        self.inner.send_string_parameters_as_unicode(enabled);
        self
    }

    /// See [`Config::multi_subnet_failover`].
    pub fn multi_subnet_failover(&mut self, multi_subnet_failover: bool) -> &mut Self {
        self.inner.multi_subnet_failover(multi_subnet_failover);
        self
    }

    /// See [`Config::packet_size`]. Unlike this builder's other setters,
    /// returns a [`Result`](crate::Result) since the packet size is range-
    /// validated; use `?` to keep chaining on success.
    pub fn packet_size(&mut self, size: u32) -> crate::Result<&mut Self> {
        self.inner.packet_size(size)?;
        Ok(self)
    }

    /// Finalizes this builder into a [`Config`]. The builder remains usable
    /// afterward (e.g. to `build()` a second, slightly different `Config`
    /// from a shared base).
    pub fn build(&self) -> Config {
        self.inner.clone()
    }
}

pub(crate) struct ServerDefinition {
    host: Option<String>,
    port: Option<u16>,
    instance: Option<String>,
}

pub(crate) trait ConfigString {
    fn dict(&self) -> &HashMap<String, String>;

    fn server(&self) -> crate::Result<ServerDefinition>;

    fn authentication(&self) -> crate::Result<AuthMethod> {
        let user = self
            .dict()
            .get("uid")
            .or_else(|| self.dict().get("username"))
            .or_else(|| self.dict().get("user"))
            .or_else(|| self.dict().get("user id"))
            .map(|s| s.as_str());

        let pw = self
            .dict()
            .get("password")
            .or_else(|| self.dict().get("pwd"))
            .map(|s| s.as_str());

        match self
            .dict()
            .get("integratedsecurity")
            .or_else(|| self.dict().get("integrated security"))
        {
            #[cfg(all(windows, feature = "winauth"))]
            Some(val) if val.to_lowercase() == "sspi" || Self::parse_bool(val)? => match (user, pw)
            {
                (None, None) => Ok(AuthMethod::Integrated),
                _ => Ok(AuthMethod::windows(user.unwrap_or(""), pw.unwrap_or(""))),
            },
            #[cfg(feature = "integrated-auth-gssapi")]
            Some(val) if val.to_lowercase() == "sspi" || Self::parse_bool(val)? => {
                Ok(AuthMethod::Integrated)
            }
            _ => Ok(AuthMethod::sql_server(user.unwrap_or(""), pw.unwrap_or(""))),
        }
    }

    fn database(&self) -> Option<String> {
        self.dict()
            .get("database")
            .or_else(|| self.dict().get("initial catalog"))
            .or_else(|| self.dict().get("databasename"))
            .map(|db| db.to_string())
    }

    fn application_name(&self) -> Option<String> {
        self.dict()
            .get("application name")
            .or_else(|| self.dict().get("applicationname"))
            .map(|name| name.to_string())
    }

    fn trust_cert(&self) -> crate::Result<bool> {
        self.dict()
            .get("trustservercertificate")
            .map(Self::parse_bool)
            .unwrap_or(Ok(false))
    }

    fn trust_cert_ca(&self) -> Option<String> {
        self.dict()
            .get("trustservercertificateca")
            .map(|ca| ca.to_string())
    }

    #[cfg(any(
        feature = "rustls",
        feature = "native-tls",
        feature = "vendored-openssl"
    ))]
    fn encrypt(&self) -> crate::Result<EncryptionLevel> {
        self.dict()
            .get("encrypt")
            .map(|val| match Self::parse_bool(val) {
                Ok(true) => Ok(EncryptionLevel::Required),
                Ok(false) => Ok(EncryptionLevel::Off),
                Err(_) if val == "DANGER_PLAINTEXT" => Ok(EncryptionLevel::NotSupported),
                Err(e) => Err(e),
            })
            .unwrap_or(Ok(EncryptionLevel::Off))
    }

    #[cfg(not(any(
        feature = "rustls",
        feature = "native-tls",
        feature = "vendored-openssl"
    )))]
    fn encrypt(&self) -> crate::Result<EncryptionLevel> {
        Ok(EncryptionLevel::NotSupported)
    }

    fn parse_bool<T: AsRef<str>>(v: T) -> crate::Result<bool> {
        match v.as_ref().trim().to_lowercase().as_str() {
            "true" | "yes" => Ok(true),
            "false" | "no" => Ok(false),
            _ => Err(crate::Error::Conversion(
                "Connection string: Not a valid boolean".into(),
            )),
        }
    }

    fn readonly(&self) -> bool {
        self.dict()
            .get("applicationintent")
            .filter(|val| *val == "ReadOnly")
            .is_some()
    }

    fn multi_subnet_failover(&self) -> crate::Result<bool> {
        self.dict()
            .get("multisubnetfailover")
            .map(Self::parse_bool)
            .unwrap_or(Ok(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_name_defaults_to_none() {
        let config = Config::new();
        assert_eq!(None, config.client_name);
    }

    #[test]
    fn client_name_can_be_set() {
        let mut config = Config::new();
        config.client_name("app-host-01");
        assert_eq!(Some("app-host-01".to_string()), config.client_name);
    }

    #[test]
    fn host_name_in_certificate_defaults_to_none() {
        let config = Config::new();
        assert_eq!(None, config.host_name_in_certificate);
    }

    #[test]
    fn host_name_in_certificate_can_be_set() {
        let mut config = Config::new();
        config.host_name_in_certificate("sql.example.com");
        assert_eq!(
            Some("sql.example.com".to_string()),
            config.host_name_in_certificate
        );
    }

    #[test]
    fn send_string_parameters_as_unicode_defaults_to_true() {
        let config = Config::new();
        assert!(config.send_string_parameters_as_unicode);
    }

    #[test]
    fn send_string_parameters_as_unicode_can_be_disabled() {
        let mut config = Config::new();
        config.send_string_parameters_as_unicode(false);
        assert!(!config.send_string_parameters_as_unicode);
    }

    #[test]
    fn multi_subnet_failover_defaults_to_false() {
        let config = Config::new();
        assert!(!config.get_multi_subnet_failover());
    }

    #[test]
    fn multi_subnet_failover_can_be_enabled() {
        let mut config = Config::new();
        config.multi_subnet_failover(true);
        assert!(config.get_multi_subnet_failover());
    }

    #[test]
    fn multi_subnet_failover_absent_from_ado_string_defaults_to_false() {
        let config =
            Config::from_ado_string("server=tcp:localhost,1433;user id=SA;password=p").unwrap();
        assert!(!config.get_multi_subnet_failover());
    }

    #[test]
    fn multi_subnet_failover_true_in_ado_string() {
        let config = Config::from_ado_string(
            "server=tcp:localhost,1433;user id=SA;password=p;MultiSubnetFailover=True",
        )
        .unwrap();
        assert!(config.get_multi_subnet_failover());
    }

    #[test]
    fn multi_subnet_failover_false_in_ado_string() {
        let config = Config::from_ado_string(
            "server=tcp:localhost,1433;user id=SA;password=p;MultiSubnetFailover=False",
        )
        .unwrap();
        assert!(!config.get_multi_subnet_failover());
    }

    #[test]
    fn multi_subnet_failover_invalid_value_in_ado_string_errors() {
        let result = Config::from_ado_string(
            "server=tcp:localhost,1433;user id=SA;password=p;MultiSubnetFailover=maybe",
        );
        assert!(result.is_err());
    }

    #[test]
    fn multi_subnet_failover_true_in_jdbc_string() {
        let config = Config::from_jdbc_string(
            "jdbc:sqlserver://localhost:1433;user=SA;password=p;multiSubnetFailover=true",
        )
        .unwrap();
        assert!(config.get_multi_subnet_failover());
    }

    #[test]
    fn packet_size_defaults_to_none() {
        let config = Config::new();
        assert_eq!(None, config.get_packet_size());
    }

    #[test]
    fn packet_size_can_be_set_within_range() {
        let mut config = Config::new();
        config.packet_size(8192).unwrap();
        assert_eq!(Some(8192), config.get_packet_size());
    }

    #[test]
    fn packet_size_accepts_the_documented_minimum() {
        let mut config = Config::new();
        config.packet_size(512).unwrap();
        assert_eq!(Some(512), config.get_packet_size());
    }

    #[test]
    fn packet_size_accepts_the_documented_maximum() {
        let mut config = Config::new();
        config.packet_size(32767).unwrap();
        assert_eq!(Some(32767), config.get_packet_size());
    }

    #[test]
    fn packet_size_rejects_below_minimum() {
        let mut config = Config::new();
        assert!(config.packet_size(511).is_err());
        assert_eq!(None, config.get_packet_size());
    }

    #[test]
    fn packet_size_rejects_above_maximum() {
        let mut config = Config::new();
        assert!(config.packet_size(32768).is_err());
        assert_eq!(None, config.get_packet_size());
    }

    #[test]
    fn packet_size_rejects_zero() {
        let mut config = Config::new();
        assert!(config.packet_size(0).is_err());
    }

    // Regression tests for prisma/tiberius#366 (ConfigBuilder): it must be a
    // pure addition alongside Config::new()'s existing direct setters, not a
    // replacement - the upstream PR removed Config::new() and every setter
    // entirely, breaking every existing caller.

    #[test]
    fn config_new_and_its_setters_still_exist() {
        // Compiles only if Config::new() and its setters are all still
        // present with their original (non-builder) signatures.
        let mut config = Config::new();
        config.host("localhost");
        config.port(1433);
        config.database("master");
        config.application_name("app");
        config.readonly(true);
        assert_eq!(Some("localhost".to_string()), config.host);
    }

    #[test]
    fn builder_produces_equivalent_config_to_direct_setters() {
        let mut direct = Config::new();
        direct.host("db.example.com");
        direct.port(1433);
        direct.database("my_db");
        direct.readonly(true);

        let built = Config::builder()
            .host("db.example.com")
            .port(1433)
            .database("my_db")
            .readonly(true)
            .build();

        assert_eq!(direct.host, built.host);
        assert_eq!(direct.port, built.port);
        assert_eq!(direct.database, built.database);
        assert_eq!(direct.readonly, built.readonly);
    }

    #[test]
    fn builder_can_be_reused_after_build() {
        let mut builder = Config::builder();
        builder.host("localhost");

        let first = builder.build();
        builder.port(9999);
        let second = builder.build();

        assert_eq!(Some("localhost".to_string()), first.host);
        assert_eq!(None, first.port);
        assert_eq!(Some("localhost".to_string()), second.host);
        assert_eq!(Some(9999), second.port);
    }

    #[test]
    fn builder_packet_size_propagates_validation_error() {
        let mut builder = Config::builder();
        assert!(builder.packet_size(0).is_err());
    }

    #[test]
    fn builder_packet_size_chains_on_success() {
        let built = Config::builder()
            .host("localhost")
            .packet_size(8192)
            .unwrap()
            .database("master")
            .build();

        assert_eq!(Some(8192), built.get_packet_size());
        assert_eq!(Some("master".to_string()), built.database);
    }
}
