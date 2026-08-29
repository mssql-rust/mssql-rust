# Tasks: upstream tiberius PR triage

Assessment of all open pull requests on [prisma/tiberius](https://github.com/prisma/tiberius/pulls)
as of 2026-08-29, done for the purpose of deciding what this fork should build.
Every PR's full diff was read (not just its description) with an emphasis on
security risk, viability, importance, and complexity. Recommendations below
double as a build queue — check items off as they land.

Already merged into this fork before this triage (for reference, not tracked
here as open work): #429, #426, #423, #419, #390, #298 (reimplemented — the
original would have broken the default build), plus direct fixes for issues
#425, #424, #418, #358, and the Config gaps mirroring bridge PR
[mssql-tiberius-bridge#50](https://github.com/saurabh500/mssql-tiberius-bridge/pull/50).

## 🚩 Reject — security or supply-chain concerns

- [ ] **#132** — named pipes example. Repoints the crate's *production*
  `tokio`/`tokio-util` dependencies at an unpinned git branch on a random
  contributor's fork. Also functionally obsolete (named pipes are in stable
  tokio now).
- [ ] **#405** — "Remove extras". Silently flips the default TLS backend
  native-tls→rustls and deletes native-tls/vendored-openssl as options
  entirely, no version bump, bundled into a 51-file/1400-line diff mixing
  four unrelated concerns. Breaks existing native-tls/vendored-openssl users
  on upgrade with no fallback.
- [ ] **#328** — named RPC + TVP support. Adds an *ungated* proc-macro
  dependency (`tvp-macro`) to plain `[dependencies]` (same mistake as #298).
  Builds a raw multi-statement batch by string-interpolating a caller-supplied
  type name (`DECLARE @P AS {db_type}`) with no quoting — SQL-injection-shaped.
  Self-admittedly incomplete TVP type coverage; conflicts with upstream.
  The named-RPC-by-name fix alone (today `RpcProcIdValue::Name` just panics)
  is worth extracting separately, without the TVP/tvp-macro portion.
- [ ] **#357** — draft, superseded by #378, adds an unnecessary ungated
  `futures` dependency for something `futures-util` (already a dep) does.

## Build now — small, safe, real value

- [x] **#411** — Zeroize SQL auth password buffers. Direct credential-handling
  hardening, clean, tested, tiny new dependency (`zeroize`). Done: `0827801`.
- [x] **#351** — Fix SSPI header type (0x10→0x11) for the NTLM continuation
  packet. One-line, spec-cited fix to a real auth wire-format bug. Applied to
  all three of this fork's SSPI-continuation call sites (winauth Integrated,
  winauth Windows, unix GSSAPI Integrated), not just the one upstream's diff
  touched. Done: see commit.
- [x] **#430** — Renew expired test cert + add podman/docker test-server
  script. Unblocks running the integration suite at all; zero source risk.
  (Supersedes the overlapping cert-fix in #405, and #241/#389, both already
  superseded by CI work already done in this fork.) The cert-renewal half was
  already moot -- this fork's cert (regenerated in `ea6bb6a`) is valid until
  2031 -- so only `docker/test-server.sh` was added, ported to this fork's
  `mssql`/`MSSQL_TEST_*` naming. Running it against a live server (Azure SQL
  Edge via rustls) validated 276 integration/bulk tests and, in the process,
  found and fixed two real bugs the unit tests alone had missed: see
  `9314a44` (money/smallmoney bulk_insert Display) and `f70bf1a` (RPC
  TYPE_INFO header for the ANSI-string-encoding feature).
- [x] **#385** — Fix `QueryStream::into_results` miscounting empty result
  sets in a multi-statement batch. Verified by hand-tracing the logic; real
  correctness bug. Added a regression test (confirmed it fails on the old
  code, passes on the fix, live). Done: `d346db2`.
- [x] **#314** — `ColumnData` `FromSql`/`ToSql`/`IntoSql` impls. Verified
  compiling and clippy-clean in an isolated worktree. Done: `622372d`.
- [x] **#359** — Bulk insert for a specified column list. Well-tested,
  additive, addresses upstream issue #311. Verified live (8 new + 80
  existing bulk tests). Done: `3c0e3e1`.
- [x] **#331** — `Row` → `TokenRow` conversion. Implemented as `From<Row> for
  TokenRow` (the PR's `Into<TokenRow> for Row` fails this fork's own `-D
  warnings` clippy gate: `from_over_into`). Verified live. Done: `b98bfc2`.

## Build with modifications — needs rework first

- [x] **Column-name bracket-escaping** (consolidates #296, #387, #388, and
  #398's bracketing piece). All four fix the same real bug (keyword/space
  column names break `bulk_insert`'s `INSERT BULK`/`SELECT TOP 0` text), with
  varying completeness: only **#387** escapes an embedded `]` correctly
  (SQL Server needs `]`→`]]`), only **#388** ships tests. Took #387's fix +
  #388's tests as one combined change. Verified live (confirmed the test
  fails without the fix, passes with it). Done: `2f33e33`.
- [x] **#413** — TDS 8.0 Strict encryption + `hostname_in_certificate`.
  Resolved the flagged open question: confirmed against Microsoft's
  current TDS 8.0 docs that Strict's PRELOGIN exchange happens *inside*
  an already-established TLS session, so the server ignores the
  ENCRYPTION byte value entirely — reusing `Required`'s wire value (as
  the PR did) is correct, not a placeholder. `hostname_in_certificate`/
  `client_name` Config fields already existed from earlier
  bridge-PR-mirroring work this session; only their connection-string
  parsing was missing, now added. TLS-before-PRELOGIN flow, ALPN
  (rustls), `encrypt=strict` parsing, unit tests. Live-verified the
  client-side failure path against a non-TDS-8.0 server (311 tests);
  positively testing a successful Strict connection needs SQL Server
  2022+/Azure SQL, and the only such image runnable here crashes under
  this arm64 host's QEMU emulation (confirmed by trying it). Done:
  `124b558`.
- [x] **#308** — Fix duplicate-`TlsStream`-symbol compile error when
  selecting a non-default TLS backend. Real, reproducible bug confirmed
  against this fork's current `tls_stream.rs`. Reapplied just the
  `tls_stream.rs`/`Cargo.toml` hunks (its chrono hunks were already applied
  here). Verified live (292 tests over rustls). Done: `64845fd`.
- [x] **#398** — `column_metadata()` + `bulk_insert_columns`. Fixed all
  7 wrong `ColumnFlag` bit positions per MS-TDS 2.2.7.4 (not just the 4
  originally flagged), fixed `bulk_insert_columns`'s filter accordingly
  (`usUpdateable` is a 2-bit value, not two flags), and added the public
  `column_metadata()` method. 12 new unit tests (verified 7 fail against
  the pre-fix bit numbering) + a live regression test for identity/computed
  column exclusion. Verified live (294 tests over rustls). Done: `e7eb6e4`,
  `7e7c40e`.
- [x] **#378** — MultiSubnetFailover. Added `Config::multi_subnet_failover`
  (+ connection-string parsing) and the concurrent-address-race refactor
  of `connect_named` for all three sql-browser backends. Config plumbing
  covered by 7 new unit tests; the connect_named race behavior itself
  can't be live-tested here (needs a real Always On listener or at least
  a Windows SQL Browser service) — verified by inspection that the
  default (non-failover) path is a mechanical, unchanged extraction of
  the pre-existing sequential loop. Done: `499f8fb`.
- [x] **#312** — SqlBulkCopyOptions / bulk insert improvements. Added
  `bulk_insert_with_options` using `enumflags2` (not a second bitflags
  dependency), fixed `KeepIdentity` to actually work (upstream defined it
  but never used it — the TDS protocol has no such WITH keyword, so it
  now controls the existing identity-column filter instead), fixed all
  4 clippy issues, and didn't carry over the PR's unrelated deletion of a
  column_data.rs test assertion (reimplemented from scratch, so there was
  nothing to restore). Live testing caught and fixed a real bug of my own
  (an empty, invalid `WITH ()` clause when only KeepIdentity was set).
  Verified live (300 tests over rustls). Done: `e03c090`.
- [x] **#400** — `packet_size` config for LOGIN7. Added
  `Config::packet_size` with the client-side range validation
  (512–32767) that was missing upstream — this matters beyond
  documentation, since the wire-framing code does an unchecked
  `packet_size - HEADER_BYTES` subtraction. 7 unit tests + a live
  connect test. Done: `6fd94b7`.
- [x] **#376** — `Decimal::into_sql`. Cherry-picked just the decimal hunk
  (dropped the unrelated log-level/already-superseded CI changes).
  Regression test confirmed it fails to compile without the fix. Verified
  live (305 tests over rustls). Done: `37248c6`.
- [x] **#366** — `ConfigBuilder`. Reimplemented as a pure addition — every
  existing `Config::new()` setter untouched, `ConfigBuilder` just forwards
  to them with `&mut Self` chaining, covering every setter including the
  ones added this session (client_name, host_name_in_certificate,
  send_string_parameters_as_unicode, multi_subnet_failover, packet_size).
  Verified live (305 tests over rustls, unaffected). Done: `7e8f9c6`.
- [x] **#408** — SSPI NTLM without Kerberos (Unix). Added `sspi-rs`
  feature. Fixed all 3 flagged issues: added unit tests for the
  (previously fully untested, for any backend) connection-string dispatch
  and for `WindowsAuth` zeroizing; deduplicated the dispatch to check its
  guard condition once instead of once per backend; changed
  `WindowsAuth.password` to `Zeroizing<String>`. Also fixed a
  `PacketHeader::sspi` vs `login` bug matching #351. Verified compiling
  clean across every feature combo including a real Windows cross-compile
  check (`--target x86_64-pc-windows-msvc`), and live for the unaffected
  SQL-auth path (305 tests). **Not verified**: the actual NTLM wire
  handshake — no NTLM-capable SQL Server is reachable from this
  environment (same gap the original PR had). Done: `6a815c7`.
- [x] **#290** — CA certificate bundle support. Reimplemented from scratch
  against current `rustls_tls_stream.rs` (0.23/aws_lc_rs/pki_types), plus
  native-tls and vendored-openssl via a new shared PEM-splitting helper
  (matching the CERTIFICATE label specifically, unlike the PR's any-BEGIN
  approach). 7 unit tests + 2 live tests (including a real multi-cert
  bundle) — verified live over both rustls and vendored-openssl (310
  tests each). Done: `01ccaa2`.
- [x] **#330** — webpki-roots support (avoid relying on native/OS cert
  store). Reimplemented from scratch against current rustls internals
  (0.23/pki_types) — the PR targeted an even older, already-removed API
  than #290 did. New `rustls-webpki-roots` feature + `Config::
  trust_webpki_roots()`. 4 unit tests + a live test proving the test
  server's self-signed cert is correctly rejected (311 tests total).
  Added to the CI Linux live-database matrix. Done: `11a2707`.

## Defer — not wrong, just low priority or already covered

- [ ] **#304** — `Row::get_column_data`. Mostly duplicates functionality
  already shipped via `Row::cells()` (merged from upstream #303).
- [ ] **#388** — column-name space fix. Superseded by the combined
  #387+#388-tests fix above; don't merge both.
- [ ] **#416** — optional serde `Serialize`/`Deserialize` feature. Solid,
  low-risk, well-tested, but still marked draft by its author — revisit once
  it's marked ready for review. Consider a doc caveat that the derived
  format isn't a stability promise.

## Already superseded — no action needed

- **#389** (bump `actions/cache` to v4) — fork's CI is already on a
  SHA-pinned v5.0.5.
- **#241** (Podman for macOS CI) — fork's CI already uses the pinned
  official `docker/setup-docker-action`.

## Suggested build order

1. #411, #351, #430, #385, #314 — all low-risk, high-confidence, do first.
2. #413 (after the PRELOGIN wire-byte check) and the combined
   #387+#388 bracket-escaping fix.
3. Work down the "needs rework" tier as time allows: #308, #398, #378, #312,
   #400, #376, #366, #408, #290, #330.
4. Skip #132, #405, #328, and #357 entirely as submitted.
