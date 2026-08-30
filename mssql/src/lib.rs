//! An asynchronous, runtime-independent, pure-rust Tabular Data Stream (TDS)
//! implementation for Microsoft SQL Server.
//!
//! This crate, `mssql`, is a fork of [Tiberius](https://github.com/prisma/tiberius),
//! the excellent TDS client originally created by Prisma and its contributors.
//! Many thanks to the Tiberius team and community for the foundation this crate
//! builds on. This fork exists to prioritize ongoing maintenance and security
//! updates, favoring small, focused changes over large rewrites, and to track
//! current SQL Server and TDS protocol versions. It is distributed under the
//! same MIT/Apache-2.0 dual license as Tiberius.
//!
//! # Connecting with async-std
//!
//! Being not bound to any single runtime, a `TcpStream` must be created
//! separately and injected to the [`Client`].
//!
//! ```no_run
//! use mssql::{Client, Config, Query, AuthMethod};
//! use async_std::net::TcpStream;
//!
//! #[async_std::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Using the builder method to construct the options.
//!     let mut config = Config::new();
//!
//!     config.host("localhost");
//!     config.port(1433);
//!
//!     // Using SQL Server authentication.
//!     config.authentication(AuthMethod::sql_server("SA", "<YourStrong@Passw0rd>"));
//!
//!     // on production, it is not a good idea to do this
//!     config.trust_cert();
//!
//!     // Taking the address from the configuration, using async-std's
//!     // TcpStream to connect to the server.
//!     let tcp = TcpStream::connect(config.get_addr()).await?;
//!
//!     // We'll disable the Nagle algorithm. Buffering is handled
//!     // internally with a `Sink`.
//!     tcp.set_nodelay(true)?;
//!
//!     // Handling TLS, login and other details related to the SQL Server.
//!     let mut client = Client::connect(config, tcp).await?;
//!
//!     // Constructing a query object with one parameter annotated with `@P1`.
//!     // This requires us to bind a parameter that will then be used in
//!     // the statement.
//!     let mut select = Query::new("SELECT @P1");
//!     select.bind(-4i32);
//!
//!     // A response to a query is a stream of data, that must be
//!     // polled to the end before querying again. Using streams allows
//!     // fetching data in an asynchronous manner, if needed.
//!     let stream = select.query(&mut client).await?;
//!
//!     // In this case, we know we have only one query, returning one row
//!     // and one column, so calling `into_row` will consume the stream
//!     // and return us the first row of the first result.
//!     let row = stream.into_row().await?;
//!
//!     assert_eq!(Some(-4i32), row.unwrap().get(0));
//!
//!     Ok(())
//! }
//! ```
//!
//! # Connecting with Tokio
//!
//! Tokio is using their own version of `AsyncRead` and `AsyncWrite` traits,
//! meaning that when wanting to use `mssql` with Tokio, their `TcpStream`
//! needs to be wrapped in Tokio's `Compat` module.
//!
//! ```no_run
//! use mssql::{Client, Config, AuthMethod};
//! use tokio::net::TcpStream;
//! use tokio_util::compat::TokioAsyncWriteCompatExt;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let mut config = Config::new();
//!
//!     config.host("localhost");
//!     config.port(1433);
//!     config.authentication(AuthMethod::sql_server("SA", "<YourStrong@Passw0rd>"));
//!     config.trust_cert(); // on production, it is not a good idea to do this
//!
//!     let tcp = TcpStream::connect(config.get_addr()).await?;
//!     tcp.set_nodelay(true)?;
//!
//!     // To be able to use Tokio's tcp, we're using the `compat_write` from
//!     // the `TokioAsyncWriteCompatExt` to get a stream compatible with the
//!     // traits from the `futures` crate.
//!     let mut client = Client::connect(config, tcp.compat_write()).await?;
//!     # client.query("SELECT @P1", &[&-4i32]).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! Connecting with [smol](https://crates.io/crates/smol) follows the same
//! shape as async-std (`smol::net::TcpStream` already implements the right
//! traits, no compat wrapper needed) — see `examples/smol.rs`.
//!
//! # Ways of querying
//!
//! `mssql` offers two ways to query the database: directly from the [`Client`]
//! with [`Client::query`] and [`Client::execute`], or additionally through
//! the [`Query`] object.
//!
//! ### With the client methods
//!
//! When the query parameters are known when writing the code, the client methods
//! are easy to use.
//!
//! ```no_run
//! # use mssql::{Client, Config, AuthMethod};
//! # use tokio::net::TcpStream;
//! # use tokio_util::compat::TokioAsyncWriteCompatExt;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let mut config = Config::new();
//! # config.host("localhost");
//! # config.port(1433);
//! # config.authentication(AuthMethod::sql_server("SA", "<YourStrong@Passw0rd>"));
//! # config.trust_cert();
//! # let tcp = TcpStream::connect(config.get_addr()).await?;
//! # tcp.set_nodelay(true)?;
//! # let mut client = Client::connect(config, tcp.compat_write()).await?;
//! let _res = client.query("SELECT @P1", &[&-4i32]).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### With the Query object
//!
//! In case of needing to pass the parameters from a dynamic collection, or if
//! wanting to pass them by-value, use the [`Query`] object.
//!
//! ```no_run
//! # use mssql::{Client, Query, Config, AuthMethod};
//! # use tokio::net::TcpStream;
//! # use tokio_util::compat::TokioAsyncWriteCompatExt;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let mut config = Config::new();
//! # config.host("localhost");
//! # config.port(1433);
//! # config.authentication(AuthMethod::sql_server("SA", "<YourStrong@Passw0rd>"));
//! # config.trust_cert();
//! # let tcp = TcpStream::connect(config.get_addr()).await?;
//! # tcp.set_nodelay(true)?;
//! # let mut client = Client::connect(config, tcp.compat_write()).await?;
//! let params = vec![String::from("foo"), String::from("bar")];
//! let mut select = Query::new("SELECT @P1, @P2, @P3");
//!
//! for param in params.into_iter() {
//!     select.bind(param);
//! }
//!
//! let _res = select.query(&mut client).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Configuration
//!
//! [`Config`] can be built either with `Config::new()` plus its setters, or
//! with the equivalent chainable [`Config::builder`] — both produce the same
//! `Config` and remain fully supported; use whichever reads better at the
//! call site:
//!
//! ```
//! # use mssql::{Config, AuthMethod};
//! let config = Config::builder()
//!     .host("localhost")
//!     .port(1433)
//!     .authentication(AuthMethod::sql_server("SA", "<YourStrong@Passw0rd>"))
//!     .trust_cert()
//!     .build();
//! # let _ = config;
//! ```
//!
//! A [`Config`] can also be parsed from an [ADO.NET connection string] (see
//! [`Config::from_ado_string`]) or a JDBC connection string (see
//! [`Config::from_jdbc_string`]), which is often the more convenient way to
//! carry settings through an environment variable or a configuration file.
//!
//! # Authentication
//!
//! `mssql` supports different [ways of authentication][`AuthMethod`] to the
//! SQL Server:
//!
//! - SQL Server authentication uses the facilities of the database to
//!   authenticate the user ([`AuthMethod::sql_server`]).
//! - On Windows, or on Unix with the `sspi-rs` feature enabled, you can
//!   authenticate with a Windows/NTLM username and password
//!   ([`AuthMethod::windows`]), or as the currently logged-in user
//!   ([`AuthMethod::Integrated`]).
//! - With the `integrated-auth-gssapi` feature enabled on Unix, it is
//!   possible to log in with the currently active Kerberos credentials
//!   ([`AuthMethod::Integrated`]) using a real ticket cache — `sspi-rs` is a
//!   pure-Rust alternative that doesn't need one, at the cost of only
//!   supporting explicit credentials, not the current-user case.
//! - AAD (Azure Active Directory) tokens are supported via
//!   [`AuthMethod::AADToken`] — see `examples/aad-auth.rs`, and the
//!   [azure_identity](https://crates.io/crates/azure_identity) crate for
//!   retrieving the token itself.
//!
//! # TLS
//!
//! When compiled with a TLS feature (`native-tls` by default, or `rustls`
//! and `vendored-openssl` as alternatives), traffic is encrypted for all
//! [`EncryptionLevel`]s except `Off`/`NotSupported`. TLS is handled with the
//! given `TcpStream`, so it works the same regardless of which runtime
//! connected it.
//!
//! Server certificate trust defaults to the operating system's certificate
//! store; [`Config::trust_cert`] disables validation entirely (development
//! only), while [`Config::trust_cert_ca`] and [`Config::trust_cert_ca_bundle`]
//! trust one specific CA certificate — from a file or from bytes already in
//! memory, respectively — in addition to the system store. With the
//! `rustls-webpki-roots` feature, `Config::trust_webpki_roots` validates
//! against Mozilla's bundled root CA list instead of the OS store, which is
//! useful in minimal containers that don't ship one.
//!
//! [`EncryptionLevel::Strict`] additionally implements TDS 8.0's mandatory
//! encryption: the TLS handshake happens *before* any TDS bytes at all
//! (closing a downgrade window the other levels leave open), and pairs with
//! [`Config::host_name_in_certificate`] for validating a certificate issued
//! for a name other than the connection address. This needs SQL Server 2022
//! or later, or Azure SQL Database/Managed Instance.
//!
//! # SQL Browser
//!
//! On Windows platforms, connecting to the SQL Server might require going through
//! the SQL Browser service to get the correct port for the named instance. This
//! feature requires either the `sql-browser-async-std`, `sql-browser-tokio`, or
//! `sql-browser-smol` feature flag to be enabled and has a bit different way of
//! connecting:
//!
//! ```no_run
//! # #[cfg(any(feature = "sql-browser-async-std", feature = "sql-browser-tokio"))]
//! use mssql::{Client, Config, AuthMethod};
//! # #[cfg(any(feature = "sql-browser-async-std", feature = "sql-browser-tokio"))]
//! use async_std::net::TcpStream;
//!
//! // An extra trait that allows connecting to a named instance with the given
//! // `TcpStream`.
//! # #[cfg(any(feature = "sql-browser-async-std", feature = "sql-browser-tokio"))]
//! use mssql::SqlBrowser;
//!
//! #[async_std::main]
//! # #[cfg(any(feature = "sql-browser-async-std", feature = "sql-browser-tokio"))]
//! async fn main() -> anyhow::Result<()> {
//!     let mut config = Config::new();
//!
//!     config.authentication(AuthMethod::sql_server("SA", "<password>"));
//!     config.host("localhost");
//!
//!     // The default port of SQL Browser
//!     config.port(1434);
//!
//!     // The name of the database server instance.
//!     config.instance_name("INSTANCE");
//!
//!     // on production, it is not a good idea to do this
//!     config.trust_cert();
//!
//!     // This will create a new `TcpStream` from `async-std`, connected to the
//!     // right port of the named instance.
//!     let tcp = TcpStream::connect_named(&config).await?;
//!
//!     // And from here on continue the connection process in a normal way.
//!     let mut client = Client::connect(config, tcp).await?;
//!     # client.query("SELECT @P1", &[&-4i32]).await?;
//!     Ok(())
//! }
//! # #[cfg(any(not(feature = "sql-browser-async-std"), not(feature = "sql-browser-tokio")))]
//! # fn main() {}
//! ```
//!
//! [`Config::multi_subnet_failover`] additionally speeds up connecting to a
//! SQL Server Always On availability group listener whose DNS name resolves
//! to addresses on multiple subnets, by racing a connection attempt against
//! every resolved address concurrently instead of trying them one at a time.
//!
//! # Bulk insert
//!
//! [`Client::bulk_insert`] efficiently loads a large number of rows into a
//! table. [`Client::bulk_insert_columns`] restricts the load to a specific
//! column list (in any order), and [`Client::bulk_insert_with_options`]
//! additionally accepts [`SqlBulkCopyOptions`] (`TABLOCK`, `KEEP_NULLS`,
//! `CHECK_CONSTRAINTS`, `FIRE_TRIGGERS`, preserving identity values) and an
//! `ORDER` hint for rows that already arrive sorted — see
//! `examples/bulk-insert-with-options.rs` and [`Client::column_metadata`]
//! for inspecting a table's columns ahead of time.
//!
//! # Other features
//!
//! - If wanting to use `mssql` with SQL Server version 2005, one must
//!   disable the `tds73` feature.
//! - [`Config::packet_size`] requests a non-default TDS packet size, which
//!   can reduce the number of network round-trips for large queries or bulk
//!   inserts; the server may negotiate a different size than requested.
//!
//! # Minimum Supported Rust Version (MSRV)
//!
//! This crate tracks current stable Rust minus two minor releases, checked
//! in CI against the exact version declared in `Cargo.toml`'s
//! `rust-version`. A change that requires a newer compiler than the policy
//! allows is a bug, not an intentional bump.
//!
//! [ADO.NET connection string]: https://docs.microsoft.com/en-us/dotnet/framework/data/adonet/connection-strings
#![cfg_attr(feature = "docs", feature(doc_cfg))]
#![recursion_limit = "512"]
#![warn(missing_docs)]
#![warn(missing_debug_implementations, rust_2018_idioms)]
#![doc(test(attr(deny(rust_2018_idioms, warnings))))]
#![doc(test(attr(allow(unused_extern_crates, unused_variables))))]

#[cfg(feature = "bigdecimal")]
pub(crate) extern crate bigdecimal_ as bigdecimal;

#[macro_use]
mod macros;

mod client;
mod from_sql;
mod query;
mod sql_read_bytes;
mod to_sql;

pub mod error;
mod result;
mod row;
mod tds;

mod bulk_options;
mod sql_browser;

pub use bulk_options::{ColumnOrderHint, SortOrder, SqlBulkCopyOption, SqlBulkCopyOptions};
pub use client::{AuthMethod, Client, Config, ConfigBuilder};
pub(crate) use error::Error;
pub use from_sql::{FromSql, FromSqlOwned};
pub use query::Query;
pub use result::*;
pub use row::{Column, ColumnType, QueryIdx, Row};
pub use sql_browser::SqlBrowser;
pub use tds::{
    codec::{
        BaseMetaDataColumn, BulkLoadRequest, ColumnData, ColumnFlag, IntoRow, MetaDataColumn,
        TokenRow, TypeInfo, TypeLength,
    },
    numeric,
    stream::QueryStream,
    time, xml, EncryptionLevel,
};
pub use to_sql::{IntoSql, ToSql};
pub use uuid::Uuid;

use sql_read_bytes::*;
use tds::codec::*;

/// An alias for a result that holds crate's error type as the error.
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn get_driver_version() -> u64 {
    env!("CARGO_PKG_VERSION")
        .splitn(6, '.')
        .enumerate()
        .fold(0u64, |acc, part| match part.1.parse::<u64>() {
            Ok(num) => acc | num << (part.0 * 8),
            _ => acc | 0 << (part.0 * 8),
        })
}
