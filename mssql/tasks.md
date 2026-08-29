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
- [ ] **#413** — TDS 8.0 Strict encryption + `hostname_in_certificate`.
  Highest-value PR in the batch: moves TLS before PRELOGIN (closes a real
  downgrade window) and fixes a genuine `NoCertVerifier` TLS 1.3 gap. Before
  merge: verify whether `EncryptionLevel::Strict` needs its own PRELOGIN wire
  byte distinct from `Required` (currently coded the same) against MS-TDS
  spec / a packet capture, not just one server's tolerance.
- [x] **#308** — Fix duplicate-`TlsStream`-symbol compile error when
  selecting a non-default TLS backend. Real, reproducible bug confirmed
  against this fork's current `tls_stream.rs`. Reapplied just the
  `tls_stream.rs`/`Cargo.toml` hunks (its chrono hunks were already applied
  here). Verified live (292 tests over rustls). Done: `64845fd`.
- [ ] **#398** — `column_metadata()` + `bulk_insert_columns`. Useful, tested
  API, but its `ColumnFlag` bitflag renumbering is only partially correct
  against the MS-TDS spec (`Updateable=0x04, Unknown=0x08, Identity=0x10,
  Computed=0x20`) — redo those bit values properly before making the type
  public.
- [ ] **#378** — MultiSubnetFailover. Complete, dependency-clean (supersedes
  draft #357). Needs basic tests on the default (non-failover) path before
  merging, since it refactors the common connect path too.
- [ ] **#312** — SqlBulkCopyOptions / bulk insert improvements. Valuable
  .NET-parity feature; fails 4 clippy style lints as submitted, and quietly
  deletes a real regression-test assertion for no stated reason (verified
  unnecessary — restoring it, all 79 `column_data` tests still pass). Fix
  both before merging.
- [ ] **#400** — `packet_size` config for LOGIN7. Real throughput win; add
  client-side range validation (512–32767), currently absent.
- [ ] **#376** — `Decimal::into_sql`. Real, small API-parity gap, bundled
  with unrelated log-level and stale CI changes — cherry-pick just the
  decimal hunk.
- [ ] **#366** — `ConfigBuilder`. The idea is fine, but as submitted it
  *removes* `Config::new()` and every existing setter — a breaking change for
  ~100% of current users. Add the builder as a pure addition; do not remove
  the existing API.
- [ ] **#408** — SSPI NTLM without Kerberos (Unix). Real auth-gap fix, but no
  test coverage for a new auth code path, duplicated GSSAPI connection-string
  parsing, and it clones the password without zeroizing (reconcile with
  #411's zeroize pattern).
- [ ] **#290** — CA certificate bundle support. Good idea, written against an
  ~18-month-old rustls API this fork has already moved past (now on
  rustls 0.23/aws_lc_rs) — reimplement the idea against current
  `rustls_tls_stream.rs`, don't apply the patch.
- [ ] **#330** — webpki-roots support (avoid relying on native/OS cert
  store). Same situation as #290 — targets rustls APIs removed in ≥0.22;
  needs a full rewrite against the current TLS internals, not a patch.

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
