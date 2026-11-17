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
- [x] **#416** — optional serde `Serialize`/`Deserialize` feature.
  Reimplemented as a pure addition, cherry-picking just the `cfg_attr`
  derives + `tests/serde.rs` (skipped its unrelated `rustls-pemfile` hunk,
  already absent from this fork's Cargo.toml). Built despite the PR still
  being draft upstream (checked 2026-08-30, unchanged since 2026-04-25) —
  the approach itself (derive-based, off-by-default via `dep:serde` +
  `uuid/serde`) held up on review. Verified: 17 new round-trip tests +
  the existing 215 lib tests, clean `clippy -D warnings` under both default
  and `all` features. Done: see commit.

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

---

# Upstream Tiberius 0.13.0: triage of merged work

Upstream moved to [tiberius-rs/tiberius](https://github.com/tiberius-rs/tiberius)
(`prisma/tiberius` now redirects there) and revived in September 2026. It
merged PRs #438–#453 (about 160 commits, about 32k changed lines, mostly by
Matthew Jackson) and released **v0.13.0** on 2026-09-24. Many thanks to the
Tiberius team. Triaged 2026-11-17 against this fork's `main` at `4e04999`.
Every commit's diff was read, not just the PR descriptions, and each fix
was checked against the fork's current source: is the fork still
vulnerable, and at which file and line. Much of #441/#442 is community PRs
this fork had already built independently (see the first half of this
file), often in a stricter form. Upstream's own hardening work is the
valuable part. Work landed on branch `upstream/tiberius-0.13-sync`.

## Done: ported, each crediting upstream in its commit message

- [x] **No-TLS `encrypt=true` in a connection string went cleartext**
  (`bd3251a`, `7fd75bb`; issue #305, which `5a070b0` only half-fixed).
  Done: `b1e9372`. Unit tests only (parse-time).
- [x] **TLS pre-login `assert_eq!` / length underflow; SSRP `&buf[3..len]`
  on a 1–2 byte UDP reply.** Both were remote panics, the first before
  certificate validation (`3e6afc3`, `fafa78a`). Done: `88dde68`.
- [x] **Numeric: `DECIMAL(38,38)` panicked on legitimate data; unvalidated
  server precision/scale; i128 overflow; `abs()` on `i128::MIN`;
  big-endian swap** (`7d13e22`, `b0cf775`, `3e6afc3`, `7803134`, `7906d55`),
  plus a fork-only fallible `rust_decimal` conversion. Done: `1b7bcdc`.
  Verified live: the new test panicked on the old code.
- [x] **ENVCHANGE PacketSize outside 512..=32767** (size 8 caused an
  infinite loop) **and ROW before COLMETADATA `unwrap()`** (`bf96855`,
  `3e6afc3`). Done: `68345bd`.
- [x] **PLP/TEXT/IMAGE allocations driven by server lengths**: u64
  `with_capacity`, zero-filling a u32 chunk, an unbounded chunk count
  (`3e6afc3`, `8255d35`). Done: `73d0eae`.
- [x] **Out-of-range date/time wire values panicked** (jiff backend:
  fork-specific; time backend: `0e977bf`, `3e6afc3`, `7803134`). Done:
  `58f2ada`.
- [x] **AAD token printed by `{:?}` on `Config`, never zeroized, sent via
  the non-zeroizing path; LOGIN7/token buffers reallocated, leaving
  credential copies behind** (`3e6afc3`, `3f47d87`, `35f2fe6`, `d290aea`).
  BREAKING: `AADToken(Zeroizing<String>)`. Also fixes the aad-auth example's
  wrong token scope. Done: `2c50da8`.
- [x] **rustls `load_native_certs().expect` / empty-store `assert!`;
  conflicting trust settings in a connection string panicked**
  (`afdf8b3`, `5885e11`, `a1abc80`). Done: `a9418c7`.
- [x] **Bulk insert: partial-row rollback** (`991702f`); **COLMETADATA
  Display `unreachable!()`** (`3e6afc3`); **reserved flag bits**
  (`e59a57f`); fork-only **ORDER-hint `]` escaping**. Done: `ffeb011`.
  Verified live: both new bulk tests fail on the old code.
- [x] **#313: in-house ADO.NET tokenizer** (`d0003ad`): `=` in an unquoted
  value now works. Corrects issues.md's "doesn't reproduce". Done:
  `58e3e0e`. Verified live with real logins.
- [x] **INSERT BULK `COLLATE`**: silent text corruption for columns whose
  collation isn't the database default (`75d7afc`). Done: `3bcc05a`.
  Verified live (Cyrillic into a CP1252 database).

## Build next: real value, small to medium, no API decisions needed

- [x] **Bulk TEXT/NTEXT (fork #352 / error 4804)**. Done on the backlog
  branch; the table name is passed via `encode_bulk_col_metadata`, so no
  public field change was needed.: upstream found the
  format: TableName as a bare US_VARCHAR, no NumParts (`103e044`, with
  `93837ef`, `74c5fa3`), plus the String→Text/NText encode arm from
  `97bbbfd`. Semver: `BaseMetaDataColumn` has pub fields, so carry the
  table name without a struct-literal break, or note it. Verify live. Then
  restore the TEXT column in `bulk_text_keeps_a_non_default_column_collation`.
- [x] **Money hardening** (done on the backlog branch): NaN/inf/out-of-range money saturates silently;
  no Numeric→Money arm; NOT NULL money columns (`FixedLen`) can't be
  bulk-inserted (`302cfe4`, `e0a65d8`, `1d91e18`, `8510d35`).
- [x] **Token-body desync** (done on the backlog branch): ERROR/INFO/LOGINACK parsed straight off the
  stream instead of within their declared Length; ORDER odd length
  (`bf96855`).
- [x] **RPC name lengths** (done on the backlog branch): proc name `as u16` truncation (`acaa4a4`) and
  the param-name `u8` counter, which overflows past 255 UTF-16 units. That
  second one is unfixed upstream too, and `ProcParam` names are
  caller-supplied.
- [x] **Poisoned connection after a cancelled multi-packet write** (done on the
  backlog branch; scoped to in-flight writes, since a bulk load dropped
  between rows was verified to be recoverable)
  (`3e6afc3`/`3f47d87` `connection.rs`, `bulk_load.rs`): new field and
  plumbing. Complements this fork's Attention-on-drop (`60bde67`).
- [x] **Bulk identifier validation** (done on the backlog branch; also allows `$`) (`7a86677`, `3f47d87`): reject
  control characters and unbalanced `]` in caller-supplied table/column
  names. Wire it into `fetch_column_metadata` once, not per entry point.
- [x] **Tolerate COLINFO / TABNAME tokens** (done on the backlog branch) (`FOR BROWSE`) by skipping by
  length. Don't port upstream's decoders (`8b6e080`, `617dc43`).
- [ ] **CI, urgent**: (Gitleaks CLI, `apt-get update`, the Linux readiness wait
  and cargo-deny: done on the backlog branch. Only the macOS lane is still
  open.) This fork's Gitleaks job failed on every PR ("License
  key is required" for an organization). Run the gitleaks CLI directly
  with a verified SHA-256, and add `.gitleaks.toml` allowlisting
  `mssql/docker/certs/` (`47331db`, `1039eb0`, `b448f43`). Also:
  `apt-get update` before `apt install` (`55be39e`), a Linux SQL Server
  readiness wait (`169091d`), and a cargo-deny job using this fork's own
  ignore list (`f823a1b`). The macOS lane currently fails in
  `setup-docker-action`; colima (`b550fc8`) is a candidate.
- [x] **Legacy CP437/CP850 collations** (done on the backlog branch, including
  the opt-in lossy code-page decoding) (currently "unsupported
  encoding") and opt-in lossy code-page decoding (`30f7b36`, `cbacb58`,
  `35605c9`). Then port upstream's CP437/CP850 bulk live test from
  `75d7afc`.
- [ ] **Small items**: ~~rustls handshake diagnostics for #327 (`610154f`)~~ (done);
  ~~connection-string docs and error hints for #333 (`0dec9cb`)~~ (done; the hint
  is in the ADO parser's error);
  ~~`Row::builder()` for #383 (`30b059c`)~~ (done); ~~case-insensitive
  `ApplicationIntent` and non-null `I16`→`i32` widening (`97bbbfd`); exact
  column match before the `r#` fallback (`bf25ac6`)~~ (done); ~~native-tls ALPN
  `tds/8.0` for Strict (`dd6d913`; async-native-tls 0.6 has
  `request_alpns`)~~ (done); ~~honour a process-installed rustls `CryptoProvider`
  with a **ring** fallback (`7235f4c`)~~ (done); `rustls-webpki` floor (`4ab275b`);
  ~~unit tests for `PacketHeader` (`0fb4067`), `into_results` (`38896ba`)
  and FeatureExtAck (`931223f`)~~ (done). Still open from this list: the
  `rustls-webpki` floor (`4ab275b`), which is a dependency declaration, so
  it belongs in a deliberate dependency-update commit.

## Needs a decision first: behaviour change or new API surface

- [x] **`handshake_timeout` / `command_timeout`** (#375/#360, `2c63330`).
  **Decided 2026-11-17: use upstream's design and defaults** (15s/30s,
  poison-on-timeout). Done on branch `upstream/tiberius-0.13-decisions`.
  Upstream defaults them to 15s/30s (BREAKING) and poisons the connection
  on timeout. Here: take the handshake timeout (`None` default, or 15s with
  a CHANGELOG note), and implement a command timeout through the existing
  Attention path rather than poisoning. **Decide** the defaults.
- [ ] **TLS trust semantics** (`2f3c4a9`, `5885e11`). The rustls
  `trust_cert_ca` rejects multi-cert files and *replaces* the roots, while
  native-tls/openssl *add* to them, and the docs claim "in addition to"
  for all three. An empty bundle silently adds nothing on
  native-tls/openssl. **Decide**: additive everywhere (widens trust) or
  replace everywhere and fix the docs. Port multi-cert and empty-bundle
  errors either way. Skip upstream's 1380-line rewrite.
- [x] **Connection-string `encrypt` default `Required`** (`3e6afc3`).
  **Decided 2026-11-17: yes.** Done on `upstream/tiberius-0.13-decisions`. It
  matches modern ADO.NET. BREAKING; this fork defaults to `Off`.
- [x] **Lift the rustls TLS 1.2-only pin** (decided 2026-11-17: yes; done on
  `upstream/tiberius-0.13-decisions`). It matters for TDS 8.0 Strict.
  The pin exists for macOS CI KeyUpdate EOFs, so re-test there first.
- [ ] **Larger additive features**, each to be built from this fork's
  source with its own review:
  - SQL_VARIANT decode/encode (`ee4450b`, `dd19eeb`, `b0cf775`). Fix the
    unchecked fixed-length size and the 16-byte numeric overflow found in
    review.
  - CLR UDT columns as `Binary` (`7c1075c`). Adding `TypeInfo::Udt` is
    breaking.
  - Typed NULL scalar parameters (`73e79d4`). Changes wire bytes; verify
    live.
  - Client-certificate (mTLS) auth (`4a212e7`).
  - Transaction Manager requests (`8338bb8`).
  - Table-valued parameters (`73e79d4`): only without the ungated macro and
    without `format!("DECLARE @P AS {db_type}")` interpolation. Same
    objection as #328.
- [ ] **Drop async-std** (`d1f7e18`): it is unmaintained, and Unreleased
  is already a breaking window.

## Skip

- `secrecy::SecretString` migration (`bacc70b`, `7084c9e`): breaking for no
  gain over this fork's `Zeroizing<String>` plus redacted `Debug`.
- `tiberius-macros` as an **ungated** proc-macro dependency (`ed9b1dd`),
  and the `Command`/`CommandStream` API, which duplicates `call_procedure`.
- ALTMETADATA/ALTROW (COMPUTE BY was removed in SQL Server 2012).
  SESSIONSTATE (`491b3b7`) and FEDAUTHINFO (`8234ac4`) decoders: unused
  here, and both have unchecked server-driven allocations or adds.
- PRELOGIN INSTOPT/TRACEID (`27d4b2a`), a default `gethostname`
  workstation name (adds `libc` and unsafe FFI), and `docsrs` cfg hygiene.
- Upstream-only CI lanes, comment trims, rustfmt-only and rename-only
  commits.
