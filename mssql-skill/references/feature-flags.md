# Feature flags

Read from `mssql/Cargo.toml`'s `[features]` table directly. If this ever
disagrees with the crate's own [`README.md`](../../mssql/README.md#feature-flags),
trust `Cargo.toml` — it's the thing that actually governs what compiles.

| Flag | Description | Default |
| --- | --- | --- |
| `tds73` | Date/time types added in TDS 7.3. Disable only if talking to SQL Server 2005, which predates 7.3. | enabled |
| `native-tls` | TLS via the operating system's own library (OpenSSL/Schannel/Security Framework). | enabled |
| `winauth` | Windows/NTLM authentication backend (via the `winauth` crate), `cfg(windows)`-gated. Backs `AuthMethod::windows` and `AuthMethod::Integrated` on Windows. | enabled |
| `rustls` | Pure-Rust TLS implementation instead of an OS library. Recommended on macOS (Security Framework doesn't work correctly with SQL Server's TLS settings there). | disabled |
| `vendored-openssl` | Statically links a vendored OpenSSL build instead of the OS's. | disabled |
| `rustls-webpki-roots` | Validate server certificates against Mozilla's bundled root CA list instead of the OS trust store — useful in minimal containers with no system store. Implies `rustls`. | disabled |
| `chrono` | Read/write date-time columns using `chrono`'s types. | disabled |
| `time` | Read/write date-time columns using the `time` crate's types (recommended over `chrono` for new code). | disabled |
| `rust_decimal` | Read/write `numeric`/`decimal` columns using `rust_decimal`'s `Decimal`. | disabled |
| `bigdecimal` | Read/write `numeric`/`decimal` columns using `bigdecimal`'s `BigDecimal`. | disabled |
| `sql-browser-async-std` | SQL Browser (named-instance resolution) for async-std's `TcpStream`. | disabled |
| `sql-browser-tokio` | SQL Browser for Tokio's `TcpStream`. | disabled |
| `sql-browser-smol` | SQL Browser for smol's `TcpStream`. | disabled |
| `integrated-auth-gssapi` | Integrated auth (log in as the current user) via a real Kerberos ticket cache (GSSAPI), Unix only. | disabled |
| `sspi-rs` | Pure-Rust NTLM for `AuthMethod::windows` on Unix, with explicit credentials, no Kerberos ticket cache needed. | disabled |
| `serde` | `Serialize`/`Deserialize` for the query result types (`Row`, `Column`, `ColumnType`, `ColumnData`, `TokenRow`, `Numeric`, and the `time`/`xml` types), so a result set can be shipped over the network as JSON. Added this cycle — see `CHANGELOG.md`'s "Unreleased" section. | disabled |
| `docs` | Enables `#![feature(doc_cfg)]`-style `doc(cfg(...))` annotations throughout the crate, so docs.rs renders "available on feature X only" badges. Nightly-only attribute; not meant to be enabled in a normal build — it's how `[package.metadata.docs.rs] features = ["all", "docs"]` builds the published docs. | disabled |
| `all` | Meta-feature enabling every backend/type/auth extra at once: `chrono`, `time`, `tds73`, all three `sql-browser-*`, `integrated-auth-gssapi`, `sspi-rs`, `rust_decimal`, `bigdecimal`, `native-tls`, `serde`. This is what CI's `--features=all` job and `cargo clippy`/`cargo test` in CONTRIBUTING.md build against. Note it does *not* include `rustls`, `vendored-openssl`, `rustls-webpki-roots`, or `winauth` — those are exercised by CI's separate `--no-default-features --features=...` matrix legs instead. | not default; opt in explicitly |

## Notes

- `default = ["tds73", "winauth", "native-tls"]` — this is what you get with
  a plain `cargo add mssql` and no `--features`/`--no-default-features`.
- Because Cargo features are additive, selecting `rustls` or
  `vendored-openssl` on top of the defaults pulls in `native-tls` too (it's
  still default-on) — this crate resolves the conflict at compile time by
  picking one actual `TlsStream` implementation, at priority `rustls` >
  `vendored-openssl` > `native-tls`, rather than failing to build. See
  `references/concepts.md` in this skill.
- `integrated-auth-gssapi` needs the GSSAPI/Kerberos headers installed on
  the build machine (`libkrb5-dev` on Debian/Ubuntu, `krb5-devel` on
  CentOS/Arch); `sspi-rs` needs nothing beyond the pure-Rust `sspi` crate.
