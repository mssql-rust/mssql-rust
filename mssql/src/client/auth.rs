use std::fmt::Debug;
use zeroize::Zeroizing;

#[derive(Clone, PartialEq, Eq)]
pub struct SqlServerAuth {
    user: String,
    password: Zeroizing<String>,
}

impl SqlServerAuth {
    pub(crate) fn into_credentials(self) -> (String, Zeroizing<String>) {
        (self.user, self.password)
    }
}

impl Debug for SqlServerAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqlServerAuth")
            .field("user", &self.user)
            .field("password", &"<HIDDEN>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
#[cfg(any(all(windows, feature = "winauth"), all(unix, feature = "sspi-rs"), doc))]
#[cfg_attr(
    feature = "docs",
    doc(cfg(any(all(windows, feature = "winauth"), all(unix, feature = "sspi-rs"))))
)]
pub struct WindowsAuth {
    pub(crate) user: String,
    pub(crate) password: Zeroizing<String>,
    pub(crate) domain: Option<String>,
}

#[cfg(any(all(windows, feature = "winauth"), all(unix, feature = "sspi-rs"), doc))]
#[cfg_attr(
    feature = "docs",
    doc(cfg(any(all(windows, feature = "winauth"), all(unix, feature = "sspi-rs"))))
)]
impl Debug for WindowsAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsAuth")
            .field("user", &self.user)
            .field("password", &"<HIDDEN>")
            .field("domain", &self.domain)
            .finish()
    }
}

/// Defines the method of authentication to the server.
#[derive(Clone, PartialEq, Eq)]
pub enum AuthMethod {
    /// Authenticate directly with SQL Server.
    SqlServer(SqlServerAuth),
    /// Authenticate with Windows credentials.
    #[cfg(any(all(windows, feature = "winauth"), all(unix, feature = "sspi-rs"), doc))]
    #[cfg_attr(
        feature = "docs",
        doc(cfg(any(all(windows, feature = "winauth"), all(unix, feature = "sspi-rs"))))
    )]
    Windows(WindowsAuth),
    /// Authenticate as the currently logged in user. On Windows uses SSPI and
    /// Kerberos on Unix platforms.
    #[cfg(any(
        all(windows, feature = "winauth"),
        all(unix, feature = "integrated-auth-gssapi"),
        doc
    ))]
    #[cfg_attr(
        feature = "docs",
        doc(cfg(any(windows, all(unix, feature = "integrated-auth-gssapi"))))
    )]
    Integrated,
    /// Authenticate with an AAD token. The token should encode an AAD user/service principal
    /// which has access to SQL Server.
    ///
    /// The token is a bearer credential, so it is held in a
    /// [`Zeroizing`] buffer (wiped on drop) and redacted from `Debug` output.
    /// Construct it with [`AuthMethod::aad_token`].
    AADToken(Zeroizing<String>),
    #[doc(hidden)]
    None,
}

impl AuthMethod {
    /// Construct a new SQL Server authentication configuration.
    pub fn sql_server(user: impl ToString, password: impl ToString) -> Self {
        Self::SqlServer(SqlServerAuth {
            user: user.to_string(),
            password: Zeroizing::new(password.to_string()),
        })
    }

    /// Construct a new Windows authentication configuration.
    #[cfg(any(all(windows, feature = "winauth"), all(unix, feature = "sspi-rs"), doc))]
    #[cfg_attr(
        feature = "docs",
        doc(cfg(any(all(windows, feature = "winauth"), all(unix, feature = "sspi-rs"))))
    )]
    pub fn windows(user: impl AsRef<str>, password: impl ToString) -> Self {
        let (domain, user) = match user.as_ref().find('\\') {
            Some(idx) => (Some(&user.as_ref()[..idx]), &user.as_ref()[idx + 1..]),
            _ => (None, user.as_ref()),
        };

        Self::Windows(WindowsAuth {
            user: user.to_string(),
            password: Zeroizing::new(password.to_string()),
            domain: domain.map(|s| s.to_string()),
        })
    }

    /// Construct a new configuration with AAD auth token.
    pub fn aad_token(token: impl ToString) -> Self {
        Self::AADToken(Zeroizing::new(token.to_string()))
    }
}

// Hand-written so the AAD bearer token is never printed: `Config` derives
// `Debug`, so `{:?}` on a `Config` (e.g. in a log line) used to include the
// token verbatim. Pattern ported from upstream Tiberius (3e6afc3 / 3f47d87).
impl Debug for AuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SqlServer(auth) => f.debug_tuple("SqlServer").field(auth).finish(),
            #[cfg(any(all(windows, feature = "winauth"), all(unix, feature = "sspi-rs"), doc))]
            Self::Windows(auth) => f.debug_tuple("Windows").field(auth).finish(),
            #[cfg(any(
                all(windows, feature = "winauth"),
                all(unix, feature = "integrated-auth-gssapi"),
                doc
            ))]
            Self::Integrated => f.write_str("Integrated"),
            Self::AADToken(_) => f.debug_tuple("AADToken").field(&"<HIDDEN>").finish(),
            Self::None => f.write_str("None"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AuthMethod;
    use zeroize::Zeroize;

    #[test]
    fn sql_server_password_can_be_consumed_and_zeroized() {
        let AuthMethod::SqlServer(auth) = AuthMethod::sql_server("sa", "secret") else {
            unreachable!();
        };

        let (user, mut password) = auth.into_credentials();

        assert_eq!("sa", user);
        assert_eq!("secret", password.as_str());

        password.zeroize();

        assert!(password.is_empty());
    }

    #[cfg(any(all(windows, feature = "winauth"), all(unix, feature = "sspi-rs")))]
    #[test]
    fn windows_auth_splits_domain_from_user() {
        let AuthMethod::Windows(auth) = AuthMethod::windows("DOMAIN\\alice", "secret") else {
            unreachable!();
        };

        assert_eq!("alice", auth.user);
        assert_eq!(Some("DOMAIN".to_string()), auth.domain);
        assert_eq!("secret", auth.password.as_str());
    }

    #[cfg(any(all(windows, feature = "winauth"), all(unix, feature = "sspi-rs")))]
    #[test]
    fn windows_auth_without_domain() {
        let AuthMethod::Windows(auth) = AuthMethod::windows("alice", "secret") else {
            unreachable!();
        };

        assert_eq!("alice", auth.user);
        assert_eq!(None, auth.domain);
    }

    #[cfg(any(all(windows, feature = "winauth"), all(unix, feature = "sspi-rs")))]
    #[test]
    fn windows_auth_password_zeroizes_on_drop() {
        // `Zeroizing<String>` zeroizes its buffer on drop; this can't
        // observe the zeroized memory directly (it's freed), but it does
        // confirm the field is actually `Zeroizing` (not a plain `String`
        // that Rust would happily drop without clearing) by exercising the
        // `Zeroize` trait's `zeroize()` method on it explicitly.
        let AuthMethod::Windows(mut auth) = AuthMethod::windows("alice", "secret") else {
            unreachable!();
        };

        auth.password.zeroize();
        assert!(auth.password.is_empty());
    }

    #[test]
    fn aad_token_is_redacted_from_debug() {
        let auth = AuthMethod::aad_token("eyJ-super-secret-token");
        let shown = format!("{auth:?}");
        assert!(!shown.contains("super-secret"), "{shown}");
        assert!(shown.contains("<HIDDEN>"), "{shown}");

        let mut config = crate::Config::new();
        config.authentication(auth);
        let shown = format!("{config:?}");
        assert!(!shown.contains("super-secret"), "{shown}");
    }

    #[test]
    fn sql_server_password_is_redacted_from_debug() {
        let auth = AuthMethod::sql_server("alice", "top-secret");
        let shown = format!("{auth:?}");

        assert!(shown.contains("alice"), "{shown}");
        assert!(shown.contains("<HIDDEN>"), "{shown}");
        assert!(!shown.contains("top-secret"), "{shown}");
    }

    #[test]
    fn none_auth_debug_is_stable() {
        assert_eq!("None", format!("{:?}", AuthMethod::None));
    }

    #[cfg(any(all(windows, feature = "winauth"), all(unix, feature = "sspi-rs")))]
    #[test]
    fn windows_auth_debug_output_hides_the_password() {
        let auth = AuthMethod::windows("DOMAIN\\alice", "secret");
        let text = format!("{auth:?}");
        assert!(text.starts_with("Windows(WindowsAuth"), "{text}");
        assert!(text.contains("alice") && text.contains("DOMAIN"), "{text}");
        assert!(text.contains("<HIDDEN>"), "{text}");
        assert!(!text.contains("secret"), "{text}");
    }

    #[cfg(any(
        all(windows, feature = "winauth"),
        all(unix, feature = "integrated-auth-gssapi")
    ))]
    #[test]
    fn integrated_auth_debug_output_names_the_method() {
        assert_eq!("Integrated", format!("{:?}", AuthMethod::Integrated));
    }
}
