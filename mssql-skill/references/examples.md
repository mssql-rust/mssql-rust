# A tour of examples/

Pointers, not copies — code moves, so read the linked file for the current,
exact version rather than trusting a remembered snippet. Run any of them
with `cargo run --example <name>` from `mssql/` (some need extra
`--features`/`--no-default-features` flags, noted below and in the file's
own doc comment).

## Connecting, by runtime

- **[`tokio.rs`](../../mssql/examples/tokio.rs)** — the baseline Tokio
  connection: `TcpStream` wrapped with `tokio_util::compat`, then
  `Client::connect`. Reach for this first if you're on Tokio.
- **[`async-std.rs`](../../mssql/examples/async-std.rs)** — the async-std
  equivalent; no compat wrapper needed since async-std's `TcpStream` already
  implements the traits `mssql` expects.
- **[`smol.rs`](../../mssql/examples/smol.rs)** — same shape as async-std,
  on the smol runtime.

## Configuration

- **[`config_builder.rs`](../../mssql/examples/config_builder.rs)** — the
  chainable `Config::builder()` API as an alternative to `Config::new()`
  plus setters. Reach for this if you prefer a fluent construction style;
  both produce the same `Config`.

## TLS and certificate trust

- **[`strict_encryption.rs`](../../mssql/examples/strict_encryption.rs)** —
  TDS 8.0 `Encrypt=Strict`, paired with `host_name_in_certificate` for real
  certificate validation. Reach for this against SQL Server 2022+/Azure SQL
  when you want to close the pre-TLS downgrade window the other encryption
  levels leave open.
- **[`ca_certificate_bundle.rs`](../../mssql/examples/ca_certificate_bundle.rs)**
  — trusting a CA certificate held in memory (e.g. from a secret manager)
  via `Config::trust_cert_ca_bundle`, rather than a file path. Supports a
  bundle of more than one PEM certificate concatenated together.
- **[`webpki_roots.rs`](../../mssql/examples/webpki_roots.rs)** — validating
  against Mozilla's bundled root CA list instead of the OS trust store, via
  the `rustls-webpki-roots` feature. Reach for this in a minimal/scratch
  container that doesn't ship a system trust store. Needs
  `--no-default-features --features=tds73,rustls-webpki-roots`; only
  validates public-CA-issued certificates, so it won't run against this
  repo's self-signed local test server.

## Authentication

- **[`windows_auth.rs`](../../mssql/examples/windows_auth.rs)** —
  Windows/NTLM with explicit credentials (`AuthMethod::windows`). Works out
  of the box on Windows; on Unix needs
  `--no-default-features --features=tds73,rustls,sspi-rs`.
- **[`aad-auth.rs`](../../mssql/examples/aad-auth.rs)** — Azure AD
  authentication via a service-principal client secret, using the
  `azure_identity`/`azure_core` crates to obtain the token and
  `AuthMethod::AADToken` to hand it to `mssql`. Reach for this when
  connecting to Azure SQL with an AAD identity instead of SQL auth.

## Bulk insert

- **[`bulk.rs`](../../mssql/examples/bulk.rs)** — the basic
  `Client::bulk_insert` path, with a progress bar. Reach for this as the
  starting point for loading a large number of rows.
- **[`bulk_insert_with_options.rs`](../../mssql/examples/bulk_insert_with_options.rs)**
  — bulk-inserting into a specific column subset with
  `Client::bulk_insert_with_options`, `SqlBulkCopyOptions` (`TABLOCK`,
  `KEEP_NULLS`, etc.), and an `ORDER` hint for pre-sorted input, mirroring
  .NET's `SqlBulkCopy`. Reach for this once the plain `bulk_insert` example
  isn't expressive enough.

## Query parameters

- **[`in_list.rs`](../../mssql/examples/in_list.rs)** — building a SQL
  `IN (...)` clause with `Query::placeholders` and `Query::bind_iter`, since
  SQL Server has no array parameter type. Reach for this whenever a query
  needs to match against a dynamically-sized list of values.

## Serialization

- **[`serde_json.rs`](../../mssql/examples/serde_json.rs)** — round-tripping
  a query result row through `serde_json` with the `serde` feature enabled
  (`cargo run --example serde_json --features serde`). Builds its row by
  hand, no live server needed. Reach for this when shipping query results
  across a network boundary (e.g. a web API forwarding rows as JSON).

## Azure redirects

- **[`redirects.rs`](../../mssql/examples/redirects.rs)** — handling
  `Error::Routing { host, port }`, which certain Azure firewall
  configurations return during login. Reach for this if you're connecting
  to Azure SQL and need to follow a redirect by opening a new `TcpStream` and
  reconnecting (there should never be more than one).
