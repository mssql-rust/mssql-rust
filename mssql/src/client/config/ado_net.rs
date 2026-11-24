use super::{ConfigString, ServerDefinition};
use std::str::FromStr;

pub(crate) struct AdoNetConfig {
    dict: super::ado_parser::AdoNetString,
}

impl FromStr for AdoNetConfig {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let dict = super::ado_parser::parse(s).map_err(|reason| {
            crate::Error::Conversion(
                format!(
                    "Connection string: {reason}. hint: if a value such as a password \
                     contains `;` or starts with a quote or `{{`, wrap it in single quotes \
                     (e.g. `password='p@ss;word'`), double quotes, or braces, doubling an \
                     enclosing quote to embed it; do not URL-encode it. For non-ASCII \
                     values, build the Config programmatically instead."
                )
                .into(),
            )
        })?;
        Ok(Self { dict })
    }
}

impl ConfigString for AdoNetConfig {
    fn dict(&self) -> &std::collections::HashMap<String, String> {
        self.dict.pairs()
    }

    fn server(&self) -> crate::Result<ServerDefinition> {
        fn parse_port(parts: &[&str]) -> crate::Result<Option<u16>> {
            Ok(match parts.first() {
                Some(s) => Some(s.parse()?),
                None => None,
            })
        }

        fn parse_server(parts: Vec<&str>) -> crate::Result<ServerDefinition> {
            if parts.is_empty() || parts.len() >= 3 {
                return Err(crate::Error::Conversion("Server value faulty.".into()));
            }

            let definition = if parts[0].contains('\\') {
                let port = parse_port(&parts[1..])?;
                let parts: Vec<&str> = parts[0].split('\\').collect();

                ServerDefinition {
                    host: Some(parts[0].replace("(local)", "localhost")),
                    port,
                    instance: Some(parts[1].into()),
                }
            } else {
                // Connect using a TCP target
                ServerDefinition {
                    host: Some(parts[0].replace("(local)", "localhost")),
                    port: parse_port(&parts[1..])?,
                    instance: None,
                }
            };

            Ok(definition)
        }

        match self
            .dict()
            .get("server")
            .or_else(|| self.dict().get("data source"))
        {
            Some(value) if value.starts_with("tcp:") => {
                parse_server(value[4..].split(',').collect())
            }
            Some(value) => parse_server(value.split(',').collect()),
            None => Ok(ServerDefinition {
                host: None,
                port: None,
                instance: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::AuthMethod;

    #[cfg(any(
        feature = "rustls",
        feature = "native-tls",
        feature = "vendored-openssl"
    ))]
    use crate::EncryptionLevel;

    #[test]
    fn server_parsing_no_browser() -> crate::Result<()> {
        let test_str = "server=tcp:my-server.com,4200";
        let ado: AdoNetConfig = test_str.parse()?;
        let server = ado.server()?;

        assert_eq!(Some("my-server.com".to_string()), server.host);
        assert_eq!(Some(4200), server.port);
        assert_eq!(None, server.instance);

        let test_str = "data source=tcp:my-server.com,4200";
        let ado: AdoNetConfig = test_str.parse()?;
        let server = ado.server()?;

        assert_eq!(Some("my-server.com".to_string()), server.host);
        assert_eq!(Some(4200), server.port);
        assert_eq!(None, server.instance);

        Ok(())
    }

    #[test]
    fn server_parsing_no_tcp() -> crate::Result<()> {
        let test_str = "server=my-server.com,4200";
        let ado: AdoNetConfig = test_str.parse()?;
        let server = ado.server()?;

        assert_eq!(Some("my-server.com".to_string()), server.host);
        assert_eq!(Some(4200), server.port);
        assert_eq!(None, server.instance);

        let test_str = "data source=my-server.com,4200";
        let ado: AdoNetConfig = test_str.parse()?;
        let server = ado.server()?;

        assert_eq!(Some("my-server.com".to_string()), server.host);
        assert_eq!(Some(4200), server.port);
        assert_eq!(None, server.instance);

        Ok(())
    }

    #[test]
    fn server_parsing_local() -> crate::Result<()> {
        let test_str = "server=tcp:(local),4200";
        let ado: AdoNetConfig = test_str.parse()?;
        let server = ado.server()?;

        assert_eq!(Some("localhost".to_string()), server.host);
        assert_eq!(Some(4200), server.port);
        assert_eq!(None, server.instance);

        let test_str = "data source=tcp:(local),4200";
        let ado: AdoNetConfig = test_str.parse()?;
        let server = ado.server()?;

        assert_eq!(Some("localhost".to_string()), server.host);
        assert_eq!(Some(4200), server.port);
        assert_eq!(None, server.instance);

        Ok(())
    }

    #[test]
    fn server_parsing_local_no_tcp() -> crate::Result<()> {
        let test_str = "server=(local),4200";
        let ado: AdoNetConfig = test_str.parse()?;
        let server = ado.server()?;

        assert_eq!(Some("localhost".to_string()), server.host);
        assert_eq!(Some(4200), server.port);
        assert_eq!(None, server.instance);

        let test_str = "data source=(local),4200";
        let ado: AdoNetConfig = test_str.parse()?;
        let server = ado.server()?;

        assert_eq!(Some("localhost".to_string()), server.host);
        assert_eq!(Some(4200), server.port);
        assert_eq!(None, server.instance);

        Ok(())
    }

    #[test]
    fn server_parsing_no_port() -> crate::Result<()> {
        let test_str = "server=tcp:my-server.com";
        let ado: AdoNetConfig = test_str.parse()?;
        let server = ado.server()?;

        assert_eq!(Some("my-server.com".to_string()), server.host);
        assert_eq!(None, server.port);
        assert_eq!(None, server.instance);

        let test_str = "server=my-server.com";
        let ado: AdoNetConfig = test_str.parse()?;
        let server = ado.server()?;

        assert_eq!(Some("my-server.com".to_string()), server.host);
        assert_eq!(None, server.port);
        assert_eq!(None, server.instance);

        Ok(())
    }

    #[test]
    fn server_parsing_with_browser() -> crate::Result<()> {
        let test_str = "server=tcp:my-server.com\\TIBERIUS";
        let ado: AdoNetConfig = test_str.parse()?;
        let server = ado.server()?;

        assert_eq!(Some("my-server.com".to_string()), server.host);
        assert_eq!(None, server.port);
        assert_eq!(Some("TIBERIUS".to_string()), server.instance);

        let test_str = "data source=tcp:my-server.com\\TIBERIUS";
        let ado: AdoNetConfig = test_str.parse()?;
        let server = ado.server()?;

        assert_eq!(Some("my-server.com".to_string()), server.host);
        assert_eq!(None, server.port);
        assert_eq!(Some("TIBERIUS".to_string()), server.instance);

        Ok(())
    }

    #[test]
    fn server_parsing_with_browser_and_port() -> crate::Result<()> {
        let test_str = "server=tcp:my-server.com\\TIBERIUS,666";
        let ado: AdoNetConfig = test_str.parse()?;
        let server = ado.server()?;

        assert_eq!(Some("my-server.com".to_string()), server.host);
        assert_eq!(Some(666), server.port);
        assert_eq!(Some("TIBERIUS".to_string()), server.instance);

        let test_str = "data source=tcp:my-server.com\\TIBERIUS,666";
        let ado: AdoNetConfig = test_str.parse()?;
        let server = ado.server()?;

        assert_eq!(Some("my-server.com".to_string()), server.host);
        assert_eq!(Some(666), server.port);
        assert_eq!(Some("TIBERIUS".to_string()), server.instance);

        Ok(())
    }

    #[test]
    fn database_parsing() -> crate::Result<()> {
        let test_str = "database=Foo";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(Some("Foo".to_string()), ado.database());

        let test_str = "databaseName=Foo";
        let jdbc: AdoNetConfig = test_str.parse()?;

        assert_eq!(Some("Foo".to_string()), jdbc.database());

        let test_str = "Initial Catalog=Foo";
        let jdbc: AdoNetConfig = test_str.parse()?;

        assert_eq!(Some("Foo".to_string()), jdbc.database());

        Ok(())
    }

    #[test]
    fn trust_cert_parsing_true() -> crate::Result<()> {
        let test_str = "TrustServerCertificate=true";
        let ado: AdoNetConfig = test_str.parse()?;

        assert!(ado.trust_cert()?);

        Ok(())
    }

    #[test]
    fn trust_cert_parsing_false() -> crate::Result<()> {
        let test_str = "TrustServerCertificate=false";
        let ado: AdoNetConfig = test_str.parse()?;

        assert!(!ado.trust_cert()?);

        Ok(())
    }

    #[test]
    fn trust_cert_parsing_yes() -> crate::Result<()> {
        let test_str = "TrustServerCertificate=yes";
        let ado: AdoNetConfig = test_str.parse()?;

        assert!(ado.trust_cert()?);

        Ok(())
    }

    #[test]
    fn trust_cert_parsing_no() -> crate::Result<()> {
        let test_str = "TrustServerCertificate=no";
        let ado: AdoNetConfig = test_str.parse()?;

        assert!(!ado.trust_cert()?);

        Ok(())
    }

    #[test]
    fn trust_cert_parsing_missing() -> crate::Result<()> {
        let test_str = "Something=foo;";
        let ado: AdoNetConfig = test_str.parse()?;

        assert!(!ado.trust_cert()?);

        Ok(())
    }

    #[test]
    fn trust_cert_parsing_faulty() -> crate::Result<()> {
        let test_str = "TrustServerCertificate=musti;";
        let ado: AdoNetConfig = test_str.parse()?;

        assert!(ado.trust_cert().is_err());

        Ok(())
    }

    #[test]
    fn trust_cert_ca_parsing_ok() -> crate::Result<()> {
        let test_str = "TrustServerCertificateCA=someca.crt;";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(Some("someca.crt".to_string()), ado.trust_cert_ca());

        Ok(())
    }

    #[test]
    fn parsing_sql_server_authentication() -> crate::Result<()> {
        let test_str = "uid=Musti; pwd=Naukio;";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(
            AuthMethod::sql_server("Musti", "Naukio"),
            ado.authentication()?
        );

        Ok(())
    }

    #[test]
    #[cfg(windows)]
    fn parsing_sspi_authentication() -> crate::Result<()> {
        let test_str = "IntegratedSecurity=SSPI;";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(AuthMethod::Integrated, ado.authentication()?);

        let test_str = "Integrated Security=SSPI;";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(AuthMethod::Integrated, ado.authentication()?);

        Ok(())
    }

    #[test]
    #[cfg(all(feature = "integrated-auth-gssapi", unix))]
    fn parsing_sspi_authentication() -> crate::Result<()> {
        let test_str = "IntegratedSecurity=true;";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(AuthMethod::Integrated, ado.authentication()?);

        let test_str = "Integrated Security=true;";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(AuthMethod::Integrated, ado.authentication()?);

        Ok(())
    }

    #[test]
    #[cfg(windows)]
    fn parsing_windows_authentication() -> crate::Result<()> {
        let test_str = "uid=Musti;pwd=Naukio; IntegratedSecurity=SSPI;";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(
            AuthMethod::windows("Musti", "Naukio"),
            ado.authentication()?
        );

        let test_str = "uid=Musti;pwd=Naukio; Integrated Security=SSPI;";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(
            AuthMethod::windows("Musti", "Naukio"),
            ado.authentication()?
        );

        Ok(())
    }

    #[test]
    fn parsing_database() -> crate::Result<()> {
        let test_str = "database=Cats;";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(Some("Cats".to_string()), ado.database());

        Ok(())
    }

    #[test]
    fn parsing_login_credentials_escaping() -> crate::Result<()> {
        let test_str = "User ID=musti; Password='abc;}45';";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(
            AuthMethod::sql_server("musti", "abc;}45"),
            ado.authentication()?
        );

        Ok(())
    }

    // Investigating #333: a password containing `$`/`%` reportedly fails
    // login even though a wire capture showed the "correct" password sent.
    // If this crate's ADO.NET-string parsing were mangling such passwords
    // (e.g. not stripping quoting correctly around a `$`), it would show up
    // right here before the value even reaches the TDS login layer.
    #[test]
    fn parsing_password_with_dollar_sign_unquoted() -> crate::Result<()> {
        let test_str = "User ID=musti; Password=Password$1337;";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(
            AuthMethod::sql_server("musti", "Password$1337"),
            ado.authentication()?
        );

        Ok(())
    }

    #[test]
    fn parsing_password_with_dollar_sign_quoted() -> crate::Result<()> {
        let test_str = "User ID=musti; Password='Password$1337';";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(
            AuthMethod::sql_server("musti", "Password$1337"),
            ado.authentication()?
        );

        Ok(())
    }

    #[test]
    fn parsing_password_with_percent_sign() -> crate::Result<()> {
        let test_str = "User ID=musti; Password=abc%123;";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(
            AuthMethod::sql_server("musti", "abc%123"),
            ado.authentication()?
        );

        Ok(())
    }

    #[test]
    #[cfg(any(
        feature = "rustls",
        feature = "native-tls",
        feature = "vendored-openssl"
    ))]
    fn encryption_parsing_on() -> crate::Result<()> {
        let test_str = "encrypt=true";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(EncryptionLevel::Required, ado.encrypt()?);

        Ok(())
    }

    #[test]
    #[cfg(any(
        feature = "rustls",
        feature = "native-tls",
        feature = "vendored-openssl"
    ))]
    fn encryption_parsing_off() -> crate::Result<()> {
        let test_str = "encrypt=false";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(EncryptionLevel::Off, ado.encrypt()?);

        Ok(())
    }

    #[test]
    #[cfg(any(
        feature = "rustls",
        feature = "native-tls",
        feature = "vendored-openssl"
    ))]
    fn encryption_parsing_plaintext() -> crate::Result<()> {
        let test_str = "encrypt=DANGER_PLAINTEXT";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(EncryptionLevel::NotSupported, ado.encrypt()?);

        Ok(())
    }

    #[test]
    #[cfg(any(
        feature = "rustls",
        feature = "native-tls",
        feature = "vendored-openssl"
    ))]
    fn encryption_parsing_missing() -> crate::Result<()> {
        let test_str = "";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(EncryptionLevel::Required, ado.encrypt()?);

        Ok(())
    }

    #[test]
    #[cfg(any(
        feature = "rustls",
        feature = "native-tls",
        feature = "vendored-openssl"
    ))]
    fn encryption_parsing_strict() -> crate::Result<()> {
        let test_str = "encrypt=strict";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(EncryptionLevel::Strict, ado.encrypt()?);

        // Case-insensitive, matching the other recognized `encrypt` values.
        let test_str = "encrypt=Strict";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(EncryptionLevel::Strict, ado.encrypt()?);

        Ok(())
    }

    #[test]
    fn host_name_in_certificate_parsing() -> crate::Result<()> {
        let test_str = "HostNameInCertificate=foo.example.com";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(
            Some("foo.example.com".to_string()),
            ado.host_name_in_certificate()
        );

        let test_str = "HostName In Certificate=foo.example.com";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(
            Some("foo.example.com".to_string()),
            ado.host_name_in_certificate()
        );

        Ok(())
    }

    #[test]
    fn client_name_parsing_from_workstation_id() -> crate::Result<()> {
        let test_str = "workstationid=meow";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(Some("meow".to_string()), ado.client_name());

        let test_str = "Workstation ID=meow";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(Some("meow".to_string()), ado.client_name());

        Ok(())
    }

    #[test]
    fn application_name_parsing() -> crate::Result<()> {
        let test_str = "Application Name=meow";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(Some("meow".into()), ado.application_name());

        let test_str = "ApplicationName=meow";
        let ado: AdoNetConfig = test_str.parse()?;

        assert_eq!(Some("meow".into()), ado.application_name());

        Ok(())
    }

    // No-TLS build: an explicit encryption request must error rather than
    // silently downgrade to plaintext (upstream Tiberius #305; tests ported
    // from tiberius-rs/tiberius bd3251a). Opting out and an omitted keyword
    // stay `NotSupported`.
    #[cfg(not(any(
        feature = "rustls",
        feature = "native-tls",
        feature = "vendored-openssl"
    )))]
    mod no_tls_encrypt {
        use super::*;

        fn parse(s: &str) -> AdoNetConfig {
            s.parse().unwrap()
        }

        #[test]
        fn encrypt_on_errors_without_tls_backend() {
            for v in ["true", "yes", "strict", "Strict"] {
                let err = parse(&format!("encrypt={v}")).encrypt().unwrap_err();
                assert!(matches!(err, crate::Error::Tls(_)), "{v}: {err:?}");
                assert!(err.to_string().contains("without a TLS backend"));
            }
        }

        #[test]
        fn encrypt_unknown_token_still_errors() {
            assert!(parse("encrypt=mandatory").encrypt().is_err());
        }

        #[test]
        fn encrypt_off_plaintext_or_missing_ok_without_tls_backend() {
            for s in [
                "encrypt=false",
                "encrypt=no",
                "encrypt=DANGER_PLAINTEXT",
                "applicationname=x",
            ] {
                assert_eq!(
                    crate::EncryptionLevel::NotSupported,
                    parse(s).encrypt().unwrap(),
                    "{s}"
                );
            }
        }
    }

    // Upstream Tiberius #313: the `connection-string` crate's ADO tokenizer
    // rejected any unquoted value containing `=` (e.g. base64 padding) with
    // "Key-value pairs must be separated by a `;`", kept `''` doubling
    // un-unescaped, trimmed spaces inside quotes, and silently folded junk
    // after a closing `}` into the value. This fork's earlier "doesn't
    // reproduce" note only tried the reporter's literal string.
    #[test]
    fn issue_313_values_with_equals_and_quotes_parse_like_ado_net() -> crate::Result<()> {
        for (s, expected) in [
            ("password=Zm9vYmFy==", "Zm9vYmFy=="),
            ("password=a=b", "a=b"),
            ("password='it''s'", "it's"),
            ("password=\"say \"\"hi\"\"\"", "say \"hi\""),
            ("password=' a '", " a "),
        ] {
            let ado: AdoNetConfig = format!("user=sa;{s}").parse()?;
            match ado.authentication()? {
                AuthMethod::SqlServer(auth) => {
                    let (_, password) = auth.into_credentials();
                    assert_eq!(expected, password.as_str(), "{s}");
                }
                other => panic!("{s}: unexpected auth {other:?}"),
            }
        }

        assert!("user=sa;password={a}b}".parse::<AdoNetConfig>().is_err());
        Ok(())
    }

    #[test]
    fn application_intent_readonly_is_case_insensitive() -> crate::Result<()> {
        for (s, expected) in [
            ("ApplicationIntent=ReadOnly", true),
            ("applicationintent=readonly", true),
            ("ApplicationIntent= READONLY ", true),
            ("ApplicationIntent=ReadWrite", false),
            ("server=x", false),
        ] {
            let ado: AdoNetConfig = s.parse()?;
            assert_eq!(expected, ado.readonly(), "{s}");
        }
        Ok(())
    }

    #[test]
    fn server_parsing_rejects_a_value_with_too_many_parts() {
        let ado: AdoNetConfig = "server=tcp:host,1433,extra".parse().unwrap();
        assert!(ado.server().is_err());
    }

    #[test]
    fn server_parsing_without_a_server_key_leaves_everything_unset() {
        let ado: AdoNetConfig = "user id=sa".parse().unwrap();
        let server = ado.server().unwrap();
        assert_eq!(None, server.host);
        assert_eq!(None, server.port);
        assert_eq!(None, server.instance);
    }
}
