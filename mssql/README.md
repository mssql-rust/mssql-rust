# mssql
[![crates.io](https://img.shields.io/crates/v/mssql.svg)](https://crates.io/crates/mssql)
[![docs.rs](https://docs.rs/mssql/badge.svg)](https://docs.rs/mssql)
[![Cargo tests](https://github.com/mssql-rust/mssql-rust/actions/workflows/test.yml/badge.svg)](https://github.com/mssql-rust/mssql-rust/actions/workflows/test.yml)
[![MSRV](https://img.shields.io/badge/rustc-1.96%2B-blue.svg)](#minimum-supported-rust-version-msrv)

A native, asynchronous Microsoft SQL Server (TDS) client for Rust.

## Quickstart

```sh
cargo add mssql
```

```rust
use mssql::{Client, Config, AuthMethod};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::new();

    config.host("localhost");
    config.port(1433);
    config.authentication(AuthMethod::sql_server("SA", "<YourStrong@Passw0rd>"));
    config.trust_cert(); // on production, validate the server certificate instead

    let tcp = TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;

    let mut client = Client::connect(config, tcp.compat_write()).await?;

    let stream = client.query("SELECT @P1", &[&-4i32]).await?;
    let row = stream.into_row().await?.unwrap();
    assert_eq!(Some(-4i32), row.get(0));

    Ok(())
}
```

See the [`examples/`](examples) directory for more, including [async-std](examples/async-std.rs)
and [smol](examples/smol.rs), the [`ConfigBuilder`](examples/config_builder.rs)
API, [`IN` lists](examples/in_list.rs), [bulk insert with options](examples/bulk_insert_with_options.rs),
[calling a stored procedure with `OUTPUT` parameters](examples/call_procedure.rs),
[TDS 8.0 Strict encryption](examples/strict_encryption.rs),
[Windows/NTLM authentication](examples/windows_auth.rs),
[CA certificate bundles](examples/ca_certificate_bundle.rs),
[webpki-roots](examples/webpki_roots.rs), [redirects](examples/redirects.rs),
[Azure AD authentication](examples/aad-auth.rs), and [serializing rows to
JSON with serde](examples/serde_json.rs) — and the crate's
[rustdoc](https://docs.rs/mssql) for the full API reference.

## Fork of Tiberius

`mssql` is a fork of [Tiberius](https://github.com/prisma/tiberius)
([crates.io](https://crates.io/crates/tiberius),
[docs.rs](https://docs.rs/tiberius/0.12.3/tiberius/)), the TDS client
originally created by Prisma and its contributors. Many thanks to the
Tiberius team and community for the work this fork builds on.

This fork exists to prioritize ongoing maintenance and security updates:

- Prioritizing support for current (most recent) versions of SQL Server and
  the TDS protocol.
- Favoring small, specific commits over large, general ones.
- Using the same free and open source licenses as Tiberius (MIT/Apache-2.0).

The `.git` history, including all of Tiberius's original commits, is
preserved in this repository.

### Goals

- A perfect implementation of the TDS protocol.
- Asynchronous network IO.
- Independent of the network protocol.
- Support for latest versions of Linux, Windows and macOS.

### Non-goals

- Connection pooling (use [bb8](https://crates.io/crates/bb8), [mobc](https://crates.io/crates/mobc), [deadpool](https://crates.io/crates/deadpool) or any of the other asynchronous connection pools)
- Query building
- Object-relational mapping

### Supported SQL Server versions

| Version | Support level | Notes                                            |
|---------|---------------|---------------------------------------------------|
|    2022 | Tested on CI  | Also supports TDS 8.0 "Strict" encryption.        |
|    2019 | Tested on CI  |                                                   |
|    2017 | Tested on CI  |                                                   |
|    2016 | Should work   |                                                   |
|    2014 | Should work   |                                                   |
|    2012 | Should work   |                                                   |
|    2008 | Should work   |                                                   |
|    2005 | Should work   | With feature flag `tds73` disabled.               |

Azure SQL Database and Azure SQL Managed Instance are also supported (and
required for TDS 8.0 Strict encryption, alongside SQL Server 2022+).

### Minimum Supported Rust Version (MSRV)

`mssql` tracks current stable Rust minus two minor releases, checked in CI
against the exact version declared in `Cargo.toml`'s `rust-version`. A
change that requires a newer compiler than the policy allows is a bug, not
an intentional bump — please open an issue.

### Feature flags

| Flag                     | Description                                                                                                                      | Default    |
|--------------------------|----------------------------------------------------------------------------------------------------------------------------------|------------|
| `tds73`                  | Support for new date and time types in TDS version 7.3. Disable if using version 7.2.                                            | `enabled`  |
| `winauth`                | Windows-native integrated authentication. Windows targets only.                                                                  | `enabled`  |
| `native-tls`             | Use operating system's TLS libraries for traffic encryption.                                                                     | `enabled`  |
| `rustls`                 | Use the builtin TLS implementation from rustls instead of linking to the operating system implementation for traffic encryption. | `disabled` |
| `vendored-openssl`       | Statically link against OpenSSL instead of dynamically linking to the operating system implementation for traffic encryption.    | `disabled` |
| `rustls-webpki-roots`    | Validate server certificates against Mozilla's bundled root CA list instead of the OS trust store. Implies `rustls`.             | `disabled` |
| `chrono`                 | Read and write date and time values using `chrono`'s types. (for greenfield, using time instead of chrono is recommended)        | `disabled` |
| `time`                   | Read and write date and time values using `time` crate types.                                                                    | `disabled` |
| `rust_decimal`           | Read and write `numeric`/`decimal` values using `rust_decimal`'s `Decimal`.                                                      | `disabled` |
| `bigdecimal`             | Read and write `numeric`/`decimal` values using `bigdecimal`'s `BigDecimal`.                                                     | `disabled` |
| `sql-browser-async-std`  | SQL Browser implementation for the `TcpStream` of async-std.                                                                     | `disabled` |
| `sql-browser-tokio`      | SQL Browser implementation for the `TcpStream` of Tokio.                                                                         | `disabled` |
| `sql-browser-smol`       | SQL Browser implementation for the `TcpStream` of smol.                                                                          | `disabled` |
| `integrated-auth-gssapi` | Support for using Integrated Auth via a real Kerberos ticket cache (GSSAPI), Unix only.                                          | `disabled` |
| `sspi-rs`                | Pure-Rust NTLM/Windows authentication with explicit credentials, without a Kerberos ticket cache, Unix only.                     | `disabled` |
| `serde`                  | `Serialize`/`Deserialize` for query result types (`Row`, `Column`, `ColumnData`, `TokenRow`, `Numeric`, time/xml types).          | `disabled` |
| `docs`                   | Internal: enables nightly `doc_cfg` annotations for docs.rs builds. Not meant to be enabled by consumers of the crate.           | `disabled` |

### Supported protocols

`mssql` does not rely on any protocol when connecting to an SQL Server instance. Instead the `Client` takes a socket that implements the `AsyncRead` and `AsyncWrite` traits from the [futures-rs](https://crates.io/crates/futures) crate.

Currently there are good async implementations for TCP in the [async-std](https://crates.io/crates/async-std), [Tokio](https://crates.io/crates/tokio) and [Smol](https://crates.io/crates/smol) projects. See [`examples/tokio.rs`](examples/tokio.rs), [`examples/async-std.rs`](examples/async-std.rs), and [`examples/smol.rs`](examples/smol.rs).

To be able to use them together with `mssql` on Windows platforms with SQL Server, you should make sure that the TCP protocol is enabled, as depending on the edition, this may not be the case. Standard and Enterprise editions will have the setting enabled by default, whereas Developer, Express editions and the Windows Internal Database feature of the Windows Server OS don't.
To enable the TCP/IP protocol you may want to use  the [server settings](https://docs.microsoft.com/en-us/sql/database-engine/configure-windows/enable-or-disable-a-server-network-protocol) the [command line](https://docs.microsoft.com/en-us/sql/powershell/how-to-enable-tcp-sqlps).
In the official [Docker image](https://hub.docker.com/_/microsoft-mssql-server) TCP is is enabled by default.

Named pipes should work by using the [NamedPipeClient](https://docs.rs/tokio/1.9.0/tokio/net/windows/named_pipe/struct.NamedPipeClient.html) from the latest Tokio versions.

The shared memory protocol is not documented and seems there are no Rust crates implementing it.

## Configuration

A [`Config`](https://docs.rs/mssql/latest/mssql/struct.Config.html) can be
built either with `Config::new()` plus its setters, or with the equivalent
chainable `Config::builder()` (see [`examples/config_builder.rs`](examples/config_builder.rs)) —
both produce the same `Config` and remain fully supported. It can also be
parsed from an [ADO.NET](https://docs.microsoft.com/en-us/dotnet/framework/data/adonet/connection-strings)
or JDBC connection string, which is often the more convenient way to carry
settings through an environment variable or a configuration file.

Beyond host/port/credentials, `Config` also covers:

- `client_name` — the workstation name sent in the login packet.
- `host_name_in_certificate` — validate a TLS certificate issued for a name
  other than the connection address (proxies, load balancers).
- `packet_size` — request a non-default TDS packet size (512–32767), which
  can reduce round-trips for large queries or bulk inserts.
- `multi_subnet_failover` — race a connection attempt against every address
  a SQL Server Always On availability group listener resolves to,
  concurrently, instead of trying them one at a time.
- `readonly` — sets the LOGIN7 `ReadOnlyIntent` flag (equivalent to
  `ApplicationIntent=ReadOnly` in an ADO.NET/JDBC connection string), so a
  connection to an Always On availability group listener gets routed to a
  read-only replica instead of the primary. `mssql` only sets the flag;
  routing itself is the listener's job — if connections keep landing on the
  primary, check the availability group's read-only routing configuration
  (`ALTER AVAILABILITY GROUP ... MODIFY REPLICA ... READ_ONLY_ROUTING_URL`)
  rather than the client.

## Encryption (TLS/SSL)

`mssql` can be set to use three different implementations of TLS connection encryption. By default it uses `native-tls`, linking to the TLS library provided by the operating system. This is a good practice and in case of security vulnerabilities, upgrading the system libraries fixes the vulnerability in `mssql` without a recompilation. On Linux we link against OpenSSL, on Windows against schannel and on macOS against Security Framework.

Alternatively one can use the `rustls` feature flag to use the Rust native TLS implementation. This way there are no dynamic dependencies to the system. This might be useful in certain installations, but requires a rebuild to update to a new TLS version. For some reasons the Security Framework on macOS does not work with SQL Server TLS settings, and on Apple platforms if needing TLS it is recommended to use `rustls` instead of `native-tls`. The other option is to use the `vendored-openssl` feature flag, that statically links against the latest OpenSSL implementation.

The crate can also be compiled without TLS support.

Because of the way default features currently work with cargo, if you select
another TLS implementation, you will get that implementation *and* the
`native-tls` implementation pulled in as dependencies (since `native-tls` is
enabled by default and additive `--features` flags don't turn it off). To
avoid ending up with two conflicting `TlsStream` implementations, this crate
picks one implementation to actually compile in, at priority `rustls` >
`vendored-openssl` > `native-tls`. This means that by default the library
uses `native-tls`, but supplying either `rustls` or `vendored-openssl` takes
priority over it. If you experience issues with the TLS handshake on macOS,
add `mssql = { version = "*", features = ["rustls"] }` to your `Cargo.toml`.

Server certificate trust defaults to the operating system's certificate
store. `Config::trust_cert()` disables validation entirely (development
only), while `Config::trust_cert_ca()` and `Config::trust_cert_ca_bundle()`
trust one specific CA certificate — from a file or from bytes already in
memory (see [`examples/ca_certificate_bundle.rs`](examples/ca_certificate_bundle.rs))
— in addition to the system store. With the `rustls-webpki-roots` feature,
`Config::trust_webpki_roots()` validates against Mozilla's bundled root CA
list instead (see [`examples/webpki_roots.rs`](examples/webpki_roots.rs)),
which is useful in minimal containers that don't ship one.

`mssql` has four runtime encryption settings:

| Encryption level | Description                                                          |
|------------------|-----------------------------------------------------------------------|
| `Required`       | All traffic is encrypted. (default)                                  |
| `Strict`         | TDS 8.0: the TLS handshake happens *before* any TDS bytes at all, closing a downgrade window the other levels leave open. Needs SQL Server 2022+ or Azure SQL. |
| `Off`            | Only the login procedure is encrypted.                                |
| `NotSupported`   | None of the traffic is encrypted.                                    |

The encryption levels can be set when connecting to the database — see
[`examples/strict_encryption.rs`](examples/strict_encryption.rs) for
`Strict`, which pairs well with `host_name_in_certificate` since
`trust_cert()` would otherwise defeat the protection Strict mode is for.

## Authentication

- SQL Server authentication (`AuthMethod::sql_server`) uses the facilities
  of the database itself.
- Windows/NTLM authentication with explicit credentials
  (`AuthMethod::windows`) works on Windows by default, or on Unix with the
  `sspi-rs` feature (pure Rust, no Kerberos ticket cache needed) — see
  [`examples/windows_auth.rs`](examples/windows_auth.rs).
- Logging in as the current user (`AuthMethod::Integrated`) works on
  Windows, or on Unix with the `integrated-auth-gssapi` feature and a real
  Kerberos ticket cache.
- Azure AD tokens (`AuthMethod::AADToken`) — see
  [`examples/aad-auth.rs`](examples/aad-auth.rs), and the
  [azure_identity](https://crates.io/crates/azure_identity) crate for
  retrieving the token itself.

### Integrated Authentication (TrustedConnection) on \*nix

With the `integrated-auth-gssapi` feature enabled, the crate requires the GSSAPI/Kerberos libraries/headers installed:
  * [CentOS](https://pkgs.org/download/krb5-devel)
  * [Arch](https://www.archlinux.org/packages/core/x86_64/krb5/)
  * [Debian](https://tracker.debian.org/pkg/krb5) (you need the -dev packages to build)
  * [Ubuntu](https://packages.ubuntu.com/bionic-updates/libkrb5-dev)
  * NixOS: Run `nix-shell shell.nix` on the repository root.
  * Mac: as of version `0.4.2` the [libgssapi](https://crates.io/crates/libgssapi) crate used for this feature now uses Apple's [GSS Framework](https://developer.apple.com/documentation/gss?language=objc) which ships with MacOS 10.14+.

Additionally, your runtime system will need to be trusted by and configured for the Active Directory domain your SQL Server is part of. In particular, you'll need to be able to get a valid TGT for your identity, via `kinit` or a keytab. This setup varies by environment and OS, but your friendly network/system administrator should be able to help figure out the specifics.

The `sspi-rs` feature is a pure-Rust alternative that needs none of the
above, at the cost of only supporting explicit Windows/NTLM credentials, not
logging in as the current user.

## Bulk insert

`Client::bulk_insert` efficiently loads a large number of rows into a table.
`Client::bulk_insert_columns` restricts the load to a specific column list
(in any order), and `Client::bulk_insert_with_options` additionally accepts
`SqlBulkCopyOptions` (`TABLOCK`, `KEEP_NULLS`, `CHECK_CONSTRAINTS`,
`FIRE_TRIGGERS`, preserving identity values) and an `ORDER` hint for rows
that already arrive sorted, mirroring .NET's `SqlBulkCopy` — see
[`examples/bulk.rs`](examples/bulk.rs) and
[`examples/bulk_insert_with_options.rs`](examples/bulk_insert_with_options.rs).
`Client::column_metadata` inspects a table's columns ahead of time.

## Stored procedures

`Client::call_procedure` calls a stored procedure by name, supporting
`OUTPUT` parameters and reading back the procedure's `RETURN` value.
Build the parameter list with `ProcParam::input`/`ProcParam::output`, then
once the returned `QueryStream` has been read (or if there's no result set
to read), call `QueryStream::into_output_params` to get an `OutputParams`
with the `OUTPUT` values and `return_status()`. An `OUTPUT` parameter's
placeholder value must be a concrete, non-`NULL` value of the right type
(e.g. `&0i32`, not `&None::<i32>`) — SQL Server rejects an untyped `NULL`
bound as `OUTPUT`, and `call_procedure` checks for this client-side before
sending anything. See [`examples/call_procedure.rs`](examples/call_procedure.rs).

## Redirects

With certain Azure firewall settings, a login might return `Error::Routing { host, port }`. This means the user must create a new `TcpStream` to the given address, and connect again — there should never be more than one redirect. See [`examples/redirects.rs`](examples/redirects.rs) for a complete, runnable example.

## Security

If you have a security issue to report, please contact
[joel@joelparkerhenderson.com](mailto:joel@joelparkerhenderson.com?subject=%5BGitHub%5D%20mssql%20Security%20Report).
