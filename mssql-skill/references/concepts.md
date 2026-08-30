# mssql concepts

The core types, the runtime-independence model, and how to choose between
the TLS backends and authentication methods. General to the crate's design —
verify anything version-specific against [docs.rs](https://docs.rs/mssql)
before treating it as final.

## The core types

- **`Config`** — everything needed to open a connection: host, port,
  authentication, encryption/trust settings, and a handful of tuning knobs
  (`client_name`, `host_name_in_certificate`, `packet_size`,
  `multi_subnet_failover`, `readonly`). Build it with `Config::new()` plus
  setters, or the equivalent chainable `Config::builder()` — both produce the
  same `Config` and are fully supported (see
  [`examples/config_builder.rs`](../../mssql/examples/config_builder.rs)).
  It can also be parsed from an ADO.NET or JDBC connection string
  (`Config::from_ado_string` / `Config::from_jdbc_string`), which is often
  the more convenient way to carry settings through an environment variable.
  `Config` does not open a socket itself — `Config::get_addr()` gives you the
  address to connect with whatever async runtime's `TcpStream` you're using.
- **`Client`** — the live connection. `Client::connect(config, stream)` takes
  ownership of an already-connected socket and drives the TDS login/TLS
  handshake over it. Once connected, `Client::query` and `Client::execute`
  run a statement with parameters known at the call site, and
  `Client::bulk_insert` / `bulk_insert_columns` / `bulk_insert_with_options`
  load rows efficiently (see [`references/examples.md`](examples.md)).
  `Client::column_metadata` inspects a table's columns ahead of time.
- **`Query`** — an alternative to the `Client` convenience methods for when
  parameters come from a dynamic collection or need to be bound by value:
  `Query::new("SELECT @P1")`, then `.bind(value)` or `.bind_iter(values)`,
  then `.query(&mut client)` / `.execute(&mut client)`. `Query::placeholders`
  builds a SQL `IN (...)` placeholder list (SQL Server has no array
  parameter), and `Query::MAX_PARAMETERS` is the server's
  2100-parameter-per-statement limit — see
  [`examples/in_list.rs`](../../mssql/examples/in_list.rs).
- **`Row`** — one row of a result set. `row.get(idx_or_name)` panics on a
  missing/out-of-range/type-mismatched column and is meant for
  "I know this schema" code; `row.try_get(...)` returns a `Result` instead.
  Both accept a numeric index or a column name. `Row::cells()` iterates the
  row's raw `ColumnData` values, and `Row::new` builds a `Row` fixture
  without a live connection (useful in tests).
- **`ColumnData`** — the typed enum a single cell decodes to (`I32`,
  `String`, `Binary`, `Numeric`, the `time`/`chrono` date-time variants,
  and so on), each variant wrapping an `Option` for nullability. `FromSql` /
  `FromSqlOwned` convert a `ColumnData` into a plain Rust value; `ToSql` /
  `IntoSql` go the other way for binding parameters. With the `serde`
  feature, `Row`, `Column`, `ColumnType`, `ColumnData`, `TokenRow`,
  `Numeric`, and the `time`/`xml` types all implement `Serialize`/
  `Deserialize`, so a result set can be shipped over the network as JSON
  (see [`examples/serde_json.rs`](../../mssql/examples/serde_json.rs)).

## Runtime independence

`mssql` does not depend on any particular async runtime or network
transport. `Client::connect` takes any socket implementing the
[`futures-rs`](https://crates.io/crates/futures) `AsyncRead`/`AsyncWrite`
traits — the caller is responsible for creating and connecting that socket.
In practice:

- **async-std** and **smol**'s `TcpStream` already implement the right
  traits directly — connect and hand it straight to `Client::connect` (see
  [`examples/async-std.rs`](../../mssql/examples/async-std.rs) and
  [`examples/smol.rs`](../../mssql/examples/smol.rs)).
- **Tokio** uses its own `AsyncRead`/`AsyncWrite` traits, so its
  `TcpStream` needs wrapping with `tokio_util::compat`'s
  `TokioAsyncWriteCompatExt::compat_write()` first (see
  [`examples/tokio.rs`](../../mssql/examples/tokio.rs) and the crate's own
  [`README.md`](../../mssql/README.md) quickstart).

This is also why SQL Browser support (resolving a named instance to a port)
is split into three features — `sql-browser-async-std`,
`sql-browser-tokio`, `sql-browser-smol` — one per runtime's own `TcpStream`.

## Choosing a TLS backend

`mssql` compiles in one of three TLS implementations. If more than one is
enabled by feature flags, exactly one is actually compiled in, at priority
**`rustls` > `vendored-openssl` > `native-tls`** — this exists specifically
so enabling an alternative backend on top of the default doesn't produce two
conflicting `TlsStream` implementations.

| Backend | Feature | When to pick it |
| --- | --- | --- |
| `native-tls` | `native-tls` (default) | Links to the OS's own TLS library (OpenSSL on Linux, Schannel on Windows, Security Framework on macOS). Security patches arrive with OS/library updates, no recompile needed. |
| `rustls` | `rustls` | Pure Rust, no dynamic system dependency. **Recommended on macOS**: Security Framework doesn't work correctly with SQL Server's TLS settings there. Needs a rebuild to pick up a new TLS version. |
| `vendored-openssl` | `vendored-openssl` | Statically links a specific OpenSSL build — useful when you need a consistent, self-contained TLS stack independent of what the OS ships. |

Certificate trust defaults to the OS certificate store. `Config::trust_cert()`
disables validation entirely (development only); `Config::trust_cert_ca()` /
`Config::trust_cert_ca_bundle()` trust one additional CA (from a file or from
in-memory bytes) alongside the system store. With `rustls-webpki-roots`
(implies `rustls`), `Config::trust_webpki_roots()` validates against
Mozilla's bundled root list instead — useful in minimal containers with no
system trust store (see
[`examples/webpki_roots.rs`](../../mssql/examples/webpki_roots.rs)).

Encryption level (`Config::encryption`) is a separate axis from the backend:
`Required` (default, all traffic encrypted), `Strict` (TDS 8.0 — the TLS
handshake happens *before* any TDS bytes, closing a downgrade window;
needs SQL Server 2022+ or Azure SQL — see
[`examples/strict_encryption.rs`](../../mssql/examples/strict_encryption.rs)),
`Off` (only login is encrypted), and `NotSupported` (nothing is encrypted).

## Choosing an authentication method

| Method | `AuthMethod` | Platform constraints |
| --- | --- | --- |
| SQL Server auth | `AuthMethod::sql_server(user, password)` | None — uses the database's own auth. |
| Windows/NTLM, explicit credentials | `AuthMethod::windows(user, password)` | Works on Windows by default (`winauth` feature, default-on). On Unix, needs the `sspi-rs` feature (pure-Rust NTLM, no Kerberos ticket cache) — see [`examples/windows_auth.rs`](../../mssql/examples/windows_auth.rs). |
| Integrated (log in as the current OS user) | `AuthMethod::Integrated` | Works on Windows by default. On Unix, needs the `integrated-auth-gssapi` feature plus a real Kerberos ticket cache (GSSAPI/`krb5` dev headers installed, a valid TGT via `kinit` or a keytab). |
| Azure AD token | `AuthMethod::AADToken(token)` | Platform-independent; the token itself is obtained separately, e.g. via the [`azure_identity`](https://crates.io/crates/azure_identity) crate — see [`examples/aad-auth.rs`](../../mssql/examples/aad-auth.rs). |

`sspi-rs` and `integrated-auth-gssapi` are not interchangeable: `sspi-rs`
only supports explicit Windows/NTLM credentials (`AuthMethod::windows`), not
logging in as the current user, while `integrated-auth-gssapi` needs a real
Kerberos ticket cache to be present and configured for the Active Directory
domain the SQL Server is part of.
