# Changes

## mssql (fork of Tiberius)

`mssql` is a fork of [Tiberius](https://github.com/prisma/tiberius), forked at
its v0.12.3 release. Many thanks to the Tiberius team and community for the
work this fork builds on. See the README for details on the fork's goals:
ongoing maintenance, security updates, and support for current SQL Server and
TDS protocol versions, favoring small, focused changes over large rewrites.

Changes below this point are entries from Tiberius's own changelog, kept for
history. New entries for `mssql` will be added above this section going
forward.

## Unreleased

## Version 1.0.0

### Added

- `Config::builder()` / `ConfigBuilder`, a chainable alternative to
  `Config::new()` plus setters — purely additive, every existing setter is
  unchanged.
- `Config::packet_size` — request a non-default TDS packet size
  (512–32767), with client-side range validation.
- `Config::multi_subnet_failover` — race a connection attempt against every
  address a SQL Server Always On availability group listener resolves to,
  concurrently, instead of trying them one at a time.
- `Config::client_name`, `Config::host_name_in_certificate`,
  `Config::send_string_parameters_as_unicode` — LOGIN7 workstation name,
  certificate hostname override for proxies/load balancers, and an escape
  hatch to send `&str`/`String` parameters as `VARCHAR` instead of always
  `NVARCHAR`.
- `Config::trust_cert_ca_bundle` — trust a CA certificate from bytes
  already in memory (a secret manager, an embedded asset), not just a file
  path.
- `rustls-webpki-roots` feature and `Config::trust_webpki_roots` — validate
  against Mozilla's bundled root CA list instead of the OS trust store,
  for containers that don't ship one.
- `EncryptionLevel::Strict` — TDS 8.0 "Strict" encryption, moving the TLS
  handshake before PRELOGIN (SQL Server 2022+ or Azure SQL).
- `sspi-rs` feature — a pure-Rust NTLM backend for `AuthMethod::Windows` on
  Unix, without needing a real Kerberos ticket cache (unlike
  `integrated-auth-gssapi`).
- `Client::bulk_insert_columns` and `Client::bulk_insert_with_options` —
  bulk-insert into a specific column list (in any order), with
  `SqlBulkCopyOptions` (`TABLOCK`, `KEEP_NULLS`, `CHECK_CONSTRAINTS`,
  `FIRE_TRIGGERS`, preserving identity values) and an `ORDER` hint,
  mirroring .NET's `SqlBulkCopy`. `bulk_insert` is now a thin wrapper
  around `bulk_insert_with_options`.
- `Client::column_metadata` — inspect a table's columns (name, type,
  nullability, identity/computed) without touching row data.
- `FromSql`/`FromSqlOwned`/`ToSql`/`IntoSql` implementations for
  `ColumnData` itself, and `IntoSql` for `rust_decimal`'s `Decimal`
  (`ToSql` already existed).
- `From<Row> for TokenRow`, to re-insert a row read from one query as bulk
  insert or statement parameters without copying each cell by hand.
- `Query::placeholders`, `Query::bind_iter`, and `Query::MAX_PARAMETERS` —
  build a SQL `IN (...)` placeholder list, bind an iterator of values in
  order, and the server's 2100-parameter-per-statement limit.
- Zeroizing of SQL Server and Windows/NTLM passwords in memory
  (`SqlServerAuth`/`WindowsAuth` now hold `Zeroizing<String>`), and of the
  LOGIN7 packet buffer after it's sent on the wire.
- `docker/test-server.sh`, a podman/docker helper for running the
  integration test suite against a local SQL Server container.
- `QueryIdx`, `TypeInfo`, and `BaseMetaDataColumn` exported from the crate
  root — all three were already `pub` internally but unreachable from
  outside the crate, blocking generic wrappers over `Row::try_get` and
  full use of `Client::column_metadata`'s return type.
- `Row::new` — a public constructor, for building `Row` fixtures in tests
  without a live connection.
- `PartialEq` for `Column`, `TokenRow`, and `Row`, for `assert_eq!`-style
  test comparisons (deliberately not `Eq`: `ColumnData` can hold `f32`/
  `f64`, and NaN breaks `Eq`'s reflexivity contract).
- `impl Debug for dyn ToSql`, so a struct holding a `&dyn ToSql`/
  `Box<dyn ToSql>` field (e.g. a query builder) can derive `Debug`.
- `serde` feature — optional `Serialize`/`Deserialize` for the query
  result types (`Row`, `Column`, `ColumnType`, `ColumnData`, `TokenRow`,
  `Numeric`, and the `time`/`xml` types), so results can be shipped over
  the network as JSON. Off by default.

### Changed

- `bulk_insert`'s identity/computed-column exclusion now correctly checks
  `usUpdateable` as the 2-bit value MS-TDS defines it as (0 = read-only,
  1 = read/write, 2 = unknown), rather than checking one bit of it in
  isolation — several `ColumnFlag` bit positions were also off by 1–2 bits
  against the MS-TDS spec (`Identity`, `Computed`, `FixedLenClrType`,
  `SparseColumnSet`, `Encrypted`, and the two `Updateable` bits).
- Selecting a non-default TLS backend (`rustls` or `vendored-openssl`)
  without `--no-default-features` no longer fails to compile with a
  duplicate-symbol error; exactly one backend now compiles in, at priority
  `rustls` > `vendored-openssl` > `native-tls`.
- `QueryStream::into_results` no longer miscounts result sets when a
  multi-statement batch includes an empty one.
- Column names that are SQL Server reserved words or contain spaces no
  longer break `bulk_insert`'s generated `INSERT BULK`/`SELECT TOP 0`
  statement text (bracket-quoted now, with `]` doubled per SQL Server's
  own escaping rule).
- Minimum Supported Rust Version policy: current stable minus two minor
  releases, checked in CI against the pinned version in `rust-version`.
- `Config::trust_cert_ca`/`ConfigBuilder::trust_cert_ca` now take
  `impl Into<PathBuf>` instead of `impl ToString`, so a path built from
  non-UTF8 bytes can be passed directly; every existing `&str`/`String`
  caller keeps compiling unchanged.
- Column-name lookup (`row.get("name")`/`try_get`) now strips a leading
  `r#` raw-identifier prefix before matching, so a column literally named
  `type` can be found via the `r#type` identifier a `FromRow`-style derive
  macro generates for a Rust keyword.
- Routine connection-lifecycle logs (TLS handshake, database/collation/
  packet-size change, login-ack version, feature-ack, server info
  messages) moved from `INFO` to `DEBUG`, so they no longer flood logs for
  short-lived pooled connections.
- PLP-encoded values (`VARCHAR(MAX)`/`NVARCHAR(MAX)`/`VARBINARY(MAX)` and
  similar) are now bulk-read instead of one byte at a time, for a
  measurable throughput improvement on large text/blob columns.

### Fixed

- Two protocol-encoding bugs found via live-server testing rather than unit
  tests alone: a panic when bulk-inserting into a `money`/`smallmoney`
  column, and RPC parameters with an overridden type (e.g. sending a
  `&str`/`String` as `VARCHAR` instead of `NVARCHAR`) corrupting the wire
  stream by omitting the `TYPE_INFO` header.
- The NTLM/GSSAPI continuation packet now uses the SSPI packet type (0x11)
  instead of LOGIN7's (0x10), across all three call sites that send one
  (`winauth` Integrated and Windows auth, and Unix GSSAPI Integrated auth).
- `PacketSize` and `Database` `ENVCHANGE` `Display` output no longer swaps
  the old and new values (MS-TDS 2.2.7.10 puts the new value first).
- Sign and padding in `Numeric`'s string formatting for negative values.
- Malformed UTF-16 in a row value is now replaced rather than causing a
  decode error.
- `DateTime2` can now be converted to `Datetimen` for `bulk_insert`.
- `Row::try_get`/`get` no longer panic on an out-of-range `usize` index;
  `try_get` now returns the same `Error::Conversion` a missing column name
  already did, and `get` (which unwraps `try_get`) documents that as its
  panic condition rather than an out-of-bounds slice access.
- A `DATE` column's `TYPE_INFO` no longer has a spurious length/scale byte
  written for it, which used to shift every subsequent column's
  `TYPE_INFO` in the same `COLMETADATA` token by one byte and corrupt
  `bulk_insert` whenever a `DATE` column wasn't last.
- `FromSql` null-widening is now symmetric across all six numeric widths
  (`u8`/`i16`/`i32`/`i64`/`f32`/`f64`): reading a NULL column as a
  different-width Rust type (e.g. a NULL `smallint` as `i32`) no longer
  fails with "cannot interpret ... as an ... value" unless a previous fix
  happened to add that specific pair.
- Bulk-inserting a value whose encoded length exceeds 65535 bytes into a
  `VARCHAR(MAX)`/`NVARCHAR(MAX)`/`VARBINARY(MAX)` column no longer fails
  with a nonsensical "exceed column limit 65535" — that check was
  comparing against the wire's MAX-column sentinel as if it were a real
  limit.
- Binding a non-finite (`NAN`/`INFINITY`) `f32`/`f64` query parameter is
  now rejected client-side with a clear error, instead of round-tripping
  to the server for a cryptic error 8023.

### Security

Two denial-of-service fixes reported by North Echo Security Research
against upstream Tiberius, both cherry-picked here:

- Seven sites in the TDS decoder responded to unexpected but
  server-controlled bytes with `panic!`, `unimplemented!`, or `.unwrap()`
  instead of returning `Err` — a malicious, compromised, or
  protocol-confused server could crash the client process
  (prisma/tiberius#424).
- `PreloginMessage::negotiated_encryption` panicked, rather than returning
  an error, when the client requested `EncryptionLevel::On` and the server
  refused with `Off`/`NotSupported` during PRELOGIN, before authentication
  and before TLS is established (prisma/tiberius#425).

Two more, found in this fork's own issue triage rather than reported
upstream:

- With no TLS feature (`rustls`/`native-tls`/`vendored-openssl`) compiled
  in, requesting `EncryptionLevel::On`/`Required` used to silently connect
  in cleartext instead of erroring — leaking the login credentials and all
  query traffic on the wire while the caller believed `encrypt=true` was
  in effect. Only `Strict` had a hard-error guard for this case; `On`/
  `Required` now get the same treatment (`Off`/`NotSupported` still
  proceed, since cleartext is what was actually requested/negotiated
  there).
- Decoding a legacy `datetime` value before 1900 (a negative day offset
  from 1900-01-01, valid back to SQL Server's minimum of 1753-01-01) used
  to panic with an integer overflow — ordinary, in-range data (a
  birthdate, a founding date) could crash the client, the same DoS class
  as the two decoder panics above.

Dependency updates and `cargo audit` findings are tracked in
`.cargo/audit.toml`; GitHub Dependabot is enabled for both scheduled
version updates and security updates.

## Version 0.12.3
- feat: improve column type accuracy (#347)
- fix: encoding of zero-length values for large varlen columns (#315)
- update tokio_rustls (#306)
- Allow iterating over the cells in a row. (#303)
- Send ReadOnlyIntent when ApplicationIntent=ReadOnly specified (#297)
- Replace encoding with encoding_rs (#285)
- Disable chrono's oldtime feature (#284)

## Version 0.12.2

- Update connection-string crate to 0.2 (#286)

## Version 0.12.1

- fix: bigdecimal conversion overflow (#271)
- Reduce futures crate dependency footprint (#270)

## Version 0.12.0

- BREAKING: Correctly convert DateTimeOffset to/from database (#269)
  Please read the [issue](https://github.com/prisma/tiberius/issues/260)
  carefully before upgrading.

## Version 0.11.8

- feat: improve column type info (#347)

## Version 0.11.7

- chore: Update connection string to 0.2 (#286)

## Version 0.11.6

- fix: bigdecimal conversion overflow (#271)

## Version 0.11.5

- Close connection explicitly (#268)

## Version 0.11.4

- Fix buffer overrun on finalize (#266)
- Correctly parse (local) server name (#259)

## Version 0.11.3

- Cleanup TokenRow public API (#255)
- Fix null values in NBC rows (#253)

## Version 0.11.2

- Fix error ordering (#248)

## Version 0.11.1

- Don't load native roots for trust-all config (#243)
- Propagate errors correctly (#247)

## Version 0.11.0

- BREAKING: bigdecimal crate upgraded to 0.3 major and has to be of
  the same major in other crates using Tiberius.
- Handle negative scale from a BigDecimal (#240)

## Version 0.10.0

- BREAKING: uuid crate upgraded to 1.0 major and has to be of the same
  major in other crates using Tiberius.

## Version 0.9.5

- Add fractional seconds precision for datetime2 (#235)

## Version 0.9.4

- Fix SQL Browser response parsing error (#229)
- Bulk uploads (#227)

## Version 0.9.3

- Enable SSL if using vendored-openssl feature (#225)

## Version 0.9.2

- Allow statically linking against OpenSSL (#222)

## Version 0.9.1

- Support AAD token authentication (#215)

## Version 0.9.0

- (BREAKING) support rustls, switch between native-tls and rustls.
  the feature flag vendored-openssl is gone. instead if needing vendored TLS,
  use feature flag rustls

## Version 0.8.0

- (BREAKING) fix: correctly decode null integers (#209)

## Version 0.7.3

- Fixing an accidentally renamed time module, that would've been a breaking change.

## Version 0.7.2

- Dynamic query interface (#196)
- Support for `time` 0.3.x (#201)
- Additional option to add custom-ca to root certificates (#203, thx @lostiniceland)

## Version 0.7.1

- Support all pre-login tokens

## Version 0.7.0

- Remove async-std from deps if using tokio
- show TokioAsyncWriteCompatExt in Client docs (#183)
- Upgrade to Rust edition 2021 (#180)

## Version 0.6.5

- Constrain UUID features and optionalize winauth dependency (smaller binaries)

## Version 0.6.4

- Use bundled bigint from bigdecimal

## Version 0.6.3

- Bignum/bigint compilation problems fixed.

## Version 0.6.2

- Improvement on waker calls. We used to wake the runtime too often, this should improve performance.

## Version 0.6.1

- SQL Browser for the smol runtime.

## Version 0.6.0

- Refactor stream handling to something more rusty (#166). This is a breaking
  change, if relying on the asynchronous stream handling of QueryResult. Please
  refer to the updated documentation.

## Version 0.5.16

- Allow setting application name per connection (#161)

## Version 0.5.15

- Split column decoding into modules (speeding up TEXT/NTEXT/IMAGE decoding a lot) (#153)

## Version 0.5.14

- Handle collations for CHAR and TEXT values (#153)

## Version 0.5.13

- Add Config parsing for "Integrated Security" (two words)
- Unified bitflag setup
- Correct default ports
- Update to enumflags2 0.7

## Version 0.5.12

- Warnings should not affect metadata fetching (#139)

## Version 0.5.11

- Fixing of all clippy warnings. This might have some performance benefits and
might also fix some weird bugs in environments where we can't guarantee the
evaluation order. (#136)
- Add info of LCID and sort id to colation errors (#138)

## Version 0.5.10

- Remove a rogue `dbg!`

## Version 0.5.9

- Set the `app_name` in LOGIN7 to `tiberius`. This allows connecting to servers
  that expect the value to not be empty (see issue #127).

## Version 0.5.8

- Try out all resolved IP addresses (#124)

## Version 0.5.7

- Set server name in the login packet (#122)

## Version 0.5.6

-  Fix for handling nullable values (#119 #121)

## Version 0.5.5 and 0.4.21

Catastropichal build failures with feature flags fixed.

## Version 0.5.4 and 0.4.20

Removed the tls feature flag to simplify dependencies. This means you will
always get a TLS-enabled build, and can disable it on runtime. This also means
we don't always compile async-std if wanting to use tokio, and so forth.

Fixes certain issues with vendored OpenSSL on macOS platforms too.

## Version 0.5.3

Changed futures-codec2 to asynchronous-codec, due to former was yanked.

## Versions 0.5.2 and 0.4.19

Introducing working TLS support on macOS platforms.

Please read the issue:

https://github.com/prisma/tiberius/issues/65

## Version 0.5.1

Internally upgrade bytes to 1.0. Should have no visible change to the apis.

## Version 0.5.0

If using Tiberius with Tokio and SQL Browser, this PR will upgrade Tokio to 1.0.

0.4 branch will be updated for a short while if needed and until the ecosystem
has completely settled on Tokio 1.0.

## Version 0.4.18

- Allow `databaseName` in connection string to define the database (#108)
- Implement reader functions for standard string data (#107)
- Fix a time conversion error (#106)

## Version 0.4.17

- Fixing error swallowing with `simple_query` and MARS (#105)
- Fixing transaction descriptor reading (#105)
- Fixing envchange token reads (#105)

## Version 0.4.16

- Handle all MARS results properly (#102)

## Version 0.4.14

- Support alternatively `BigNumber` when dealing with numeric values.
- Document feature flags

## Version 0.4.13

- Realizing UTF-16 works just fine with SQL Server. Reverting the UCS2, but
  still keeping the faster writes.

## Version 0.4.12

*SKIP this, go directly to 0.4.13*

- A typo fix in README (#94)
- Faster string writes with better length handling. UCS2 for writes (#95).

## Version 0.4.11

- Allow disabling TLS in connection string (#89)
- Use connection-string for ado.net parsing (#91)
- Handle JDBC connection strings (#92)

## Version 0.4.10

- Handling nullable int values, fix for #78 (#80)
- Reflect tweaks to upstream libgssapi crate (#81)
- Skip default features in libgssapi (for macOS support)
- Handle env change Routing request (#87)

## Version 0.4.9

- BREAKING: `AuthMethod::WindowsIntegrated` renamed to `AuthMethod::Integrated`.
- Use GSSAPI for IntegratedSecurity on Unix platforms
- Fix module docs for examples
- Make `packet_id` wrapping explicit
- Add DNS feature to Tokio

## Version 0.4.8

- BREAKING: `ColumnData::I8(i8)` is now `ColumnData::U8(u8)` due to misunderstanding how `tinyint` works. (#71)
- Skip any received `done_rows` amounts and avoid creating extra resultsets (#67)
- Actually run the chrono tests (#72)
- Fix GUID byte ordering (#69)
- Fix null time/datetime2/datetimeoffset handling (#73)
- Null image data should be `Binary`, not `String`

## Version 0.4.7

- Pass hostname to TLS handshake, allowing usage with AzureSQL using
  `TrustServerCertificate=no`
  ([#62](https://github.com/prisma/tiberius/pull/62))

## Version 0.4.5

- Documenting type conversions and re-exporting chrono types
  ([#60](https://github.com/prisma/tiberius/pull/60))

## Version 0.4.4

- Fixing multi-part table names in `IMAGE`, `TEXT` and `NTEXT` column metadata
  ([#58](https://github.com/prisma/tiberius/pull/58))

## Version 0.4.3

- Starting transactions with `simple_query` now works without crashing
  ([#55](https://github.com/prisma/tiberius/pull/55))

## Version 0.4.2

- Fixing old and wrong `ExecuteResult` docs
- Adding `rows_affected` method to `ExecuteResult`

## Version 0.4.1

- Add all feature flags for docs.rs build

## Version 0.4.0

- A complete rewrite from 0.3.0
- Not bound to Tokio anymore, independent of the runtime
- Support for many more types
- Async/await, futures 0.3
