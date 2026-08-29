# mssql
[![crates.io](https://img.shields.io/crates/v/mssql.svg)](https://crates.io/crates/mssql)
[![docs.rs](https://docs.rs/mssql/badge.svg)](https://docs.rs/mssql)
[![Cargo tests](https://github.com/mssql-rust/mssql-rust/actions/workflows/test.yml/badge.svg)](https://github.com/mssql-rust/mssql-rust/actions/workflows/test.yml)

A native Microsoft SQL Server (TDS) client for Rust.

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

| Version | Support level | Notes                               |
|---------|---------------|-------------------------------------|
|    2022 | Tested on CI  |                                     |
|    2019 | Tested on CI  |                                     |
|    2017 | Tested on CI  |                                     |
|    2016 | Should work   |                                     |
|    2014 | Should work   |                                     |
|    2012 | Should work   |                                     |
|    2008 | Should work   |                                     |
|    2005 | Should work   | With feature flag `tds73` disabled. |

### Feature flags

| Flag                     | Description                                                                                                                      | Default    |
|--------------------------|----------------------------------------------------------------------------------------------------------------------------------|------------|
| `tds73`                  | Support for new date and time types in TDS version 7.3. Disable if using version 7.2.                                            | `enabled`  |
| `native-tls`             | Use operating system's TLS libraries for traffic encryption.                                                                     | `enabled`  |
| `rustls`                 | Use the builtin TLS implementation from rustls instead of linking to the operating system implementation for traffic encryption. | `disabled` |
| `vendored-openssl`       | Statically link against OpenSSL instead of dynamically linking to the operating system implementation for traffic encryption.    | `disabled` |
| `chrono`                 | Read and write date and time values using `chrono`'s types. (for greenfield, using time instead of chrono is recommended)        | `disabled` |
| `time`                   | Read and write date and time values using `time` crate types.                                                                    | `disabled` |
| `rust_decimal`           | Read and write `numeric`/`decimal` values using `rust_decimal`'s `Decimal`.                                                      | `disabled` |
| `bigdecimal`             | Read and write `numeric`/`decimal` values using `bigdecimal`'s `BigDecimal`.                                                     | `disabled` |
| `sql-browser-async-std`  | SQL Browser implementation for the `TcpStream` of async-std.                                                                     | `disabled` |
| `sql-browser-tokio`      | SQL Browser implementation for the `TcpStream` of Tokio.                                                                         | `disabled` |
| `sql-browser-smol`       | SQL Browser implementation for the `TcpStream` of smol.                                                                          | `disabled` |
| `integrated-auth-gssapi` | Support for using Integrated Auth via GSSAPI                                                                                     | `disabled` |

### Supported protocols

`mssql` does not rely on any protocol when connecting to an SQL Server instance. Instead the `Client` takes a socket that implements the `AsyncRead` and `AsyncWrite` traits from the [futures-rs](https://crates.io/crates/futures) crate.

Currently there are good async implementations for TCP in the [async-std](https://crates.io/crates/async-std), [Tokio](https://crates.io/crates/tokio) and [Smol](https://crates.io/crates/smol) projects.

To be able to use them together with `mssql` on Windows platforms with SQL Server, you should make sure that the TCP protocol is enabled, as depending on the edition, this may not be the case. Standard and Enterprise editions will have the setting enabled by default, whereas Developer, Express editions and the Windows Internal Database feature of the Windows Server OS don't.
To enable the TCP/IP protocol you may want to use  the [server settings](https://docs.microsoft.com/en-us/sql/database-engine/configure-windows/enable-or-disable-a-server-network-protocol) the [command line](https://docs.microsoft.com/en-us/sql/powershell/how-to-enable-tcp-sqlps).
In the official [Docker image](https://hub.docker.com/_/microsoft-mssql-server) TCP is is enabled by default.

Named pipes should work by using the [NamedPipeClient](https://docs.rs/tokio/1.9.0/tokio/net/windows/named_pipe/struct.NamedPipeClient.html) from the latest Tokio versions.

The shared memory protocol is not documented and seems there are no Rust crates implementing it.

### Encryption (TLS/SSL)

`mssql` can be set to use two different implementations of TLS connection encryption. By default it uses `native-tls`, linking to the TLS library provided by the operating system. This is a good practice and in case of security vulnerabilities, upgrading the system libraries fixes the vulnerability in `mssql` without a recompilation. On Linux we link against OpenSSL, on Windows against schannel and on macOS against Security Framework.

Alternatively one can use the `rustls` feature flag to use the Rust native TLS implementation. This way there are no dynamic dependencies to the system. This might be useful in certain installations, but requires a rebuild to update to a new TLS version. For some reasons the Security Framework on macOS does not work with SQL Server TLS settings, and on Apple platforms if needing TLS it is recommended to use `rustls` instead of `native-tls`. The other option is to use the `vendored-openssl` feature flag, that statically links against the latest OpenSSL implementation.

The crate can also be compiled without TLS support, but not with both features enabled at the same time.

`mssql` has three runtime encryption settings:

| Encryption level | Description                                      |
|------------------|--------------------------------------------------|
| `Required`       | All traffic is encrypted. (default)              |
| `Off`            | Only the login procedure is encrypted.           |
| `NotSupported`   | None of the traffic is encrypted.                |

The encryption levels can be set when connecting to the database.

### Integrated Authentication (TrustedConnection) on \*nix

With the `integrated-auth-gssapi` feature enabled, the crate requires the GSSAPI/Kerberos libraries/headers installed:
  * [CentOS](https://pkgs.org/download/krb5-devel)
  * [Arch](https://www.archlinux.org/packages/core/x86_64/krb5/)
  * [Debian](https://tracker.debian.org/pkg/krb5) (you need the -dev packages to build)
  * [Ubuntu](https://packages.ubuntu.com/bionic-updates/libkrb5-dev)
  * NixOS: Run `nix-shell shell.nix` on the repository root.
  * Mac: as of version `0.4.2` the [libgssapi](https://crates.io/crates/libgssapi) crate used for this feature now uses Apple's [GSS Framework](https://developer.apple.com/documentation/gss?language=objc) which ships with MacOS 10.14+.

Additionally, your runtime system will need to be trusted by and configured for the Active Directory domain your SQL Server is part of. In particular, you'll need to be able to get a valid TGT for your identity, via `kinit` or a keytab. This setup varies by environment and OS, but your friendly network/system administrator should be able to help figure out the specifics.

## Redirects

With certain Azure firewall settings, a login might return `Error::Routing { host, port }`. This means the user must create a new `TcpStream` to the given address, and connect again.

A simple connection procedure would then be:

```rust
use mssql::{Client, Config, AuthMethod, error::Error};
use tokio_util::compat::TokioAsyncWriteCompatExt;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::new();

    config.host("0.0.0.0");
    config.port(1433);
    config.authentication(AuthMethod::sql_server("SA", "<Mys3cureP4ssW0rD>"));

    let tcp = TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;

    let client = match Client::connect(config, tcp.compat_write()).await {
        // Connection successful.
        Ok(client) => client,
        // The server wants us to redirect to a different address
        Err(Error::Routing { host, port }) => {
            let mut config = Config::new();

            config.host(&host);
            config.port(port);
            config.authentication(AuthMethod::sql_server("SA", "<Mys3cureP4ssW0rD>"));

            let tcp = TcpStream::connect(config.get_addr()).await?;
            tcp.set_nodelay(true)?;

            // we should not have more than one redirect, so we'll short-circuit here.
            Client::connect(config, tcp.compat_write()).await?
        }
        Err(e) => Err(e)?,
    };

    Ok(())
}
```

## Security

If you have a security issue to report, please contact
[joel@joelparkerhenderson.com](mailto:joel@joelparkerhenderson.com?subject=%5BGitHub%5D%20mssql%20Security%20Report).
