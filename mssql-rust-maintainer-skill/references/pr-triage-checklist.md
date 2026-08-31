# Triaging the next upstream PR or issue

A reusable version of the discipline evident in
[`tasks.md`](../../mssql/tasks.md) (PRs) and
[`issues.md`](../../mssql/issues.md) (issues) — both triages were done
against `prisma/tiberius` for the purpose of deciding what this fork should
build, and follow the same shape. Apply this checklist to the next PR or
issue that comes up, and add the outcome to the relevant file's build queue
under the matching section, following that file's existing entry style
(short, cites the PR/issue number, states what was verified and what
wasn't, links the landing commit once done).

## 1. Read the whole thing

- Fetch the full PR diff or issue body **and its comments** — not just the
  title or a summary. `gh pr view <n> --comments` /
  `gh issue view <n> --comments` against `prisma/tiberius`, or the fork's
  own tracker if the item originates here.
- Cross-check against this fork's **current** source and git history, not
  against what upstream looked like when the PR/issue was filed — this fork
  has already landed fixes upstream hasn't (check `CHANGELOG.md`'s
  "Unreleased" section and `tasks.md`'s "Already merged... before this
  triage" list first, since the item may already be moot).

## 2. Security and supply-chain risk first

Reject on sight, regardless of the feature's merit, if the change:

- Repoints a dependency at an unpinned git branch/fork instead of a
  published, versioned release.
- Adds an ungated dependency (especially a proc-macro) to plain
  `[dependencies]` when it should be feature-gated and optional.
- Silently changes a security-relevant default (TLS backend, encryption
  level, certificate trust) without a major/breaking version bump and a
  documented migration path.
- Builds SQL or wire-protocol text by string interpolation of caller-supplied
  values without quoting/escaping (SQL-injection-shaped, even in a "trusted"
  internal path).
- Requests a downgrade in security posture to support an end-of-life target
  (e.g. TLS 1.0, disabled certificate verification, SQL Server 2000/2008)
  — conflicts with this fork's maintenance/security mission by design; see
  `issues.md`'s Reject section for the actual precedents.

## 3. Scope and mixing

- Does the diff mix more than one concern (a real fix bundled with unrelated
  CI changes, log-level tweaks, or a deletion of an existing test
  assertion)? If so, plan to cherry-pick just the relevant hunk rather than
  merging the whole diff — `tasks.md`'s `#308` and `#376` entries are the
  pattern to follow.
- Does another open PR/issue fix the same root cause? Check for duplicates
  before building — several `tasks.md`/`issues.md` entries consolidate two
  to four reports of the same bug (`#373`+`#410`, `#258`+`#262`,
  `#397`+`#403`) into one fix, taking the best parts of each rather than
  merging more than one independently.
- Is this already superseded by work this fork has already done (a newer
  dependency version, a different fix already landed)? Say so and skip it
  rather than reapplying stale work.

## 4. Correctness review

- Trace the logic by hand for anything protocol- or encoding-related — this
  fork's history includes real bugs found only by hand-tracing
  (`QueryStream::into_results`'s empty-result-set miscount) or by comparing
  against the MS-TDS spec byte-for-byte (the `ColumnFlag` bit-position
  fixes, the DATE `TYPE_INFO` byte-shift).
- If the PR/issue includes a patch sketch or reference fix, decide whether
  to port it directly or reimplement against current source — reimplement
  when the target code has moved on enough that the patch no longer applies
  cleanly (rustls's internal API is a repeat offender here: `#290`, `#330`
  both had to be reimplemented from scratch against current
  `rustls_tls_stream.rs`).

## 5. Test it, and verify the test can fail

- Add a regression test. Before trusting it, confirm it actually fails
  against the old (unfixed) code and passes against the fix — several
  entries call this out explicitly (`#385`, the DATE fix, `#322`'s
  encoded-length check).
- For anything touching the wire protocol, bulk insert, or TLS: verify live
  against a real SQL Server via `docker/test-server.sh`, not unit tests
  alone — this fork's own history is that live testing has found real bugs
  unit tests missed (money/smallmoney bulk-insert panic, an RPC `TYPE_INFO`
  header omission, both found while validating `#430`). See
  [`verification-commands.md`](verification-commands.md) for the exact
  commands.
- Run the standard verification matrix (build, clippy, the feature-flag
  combinations relevant to the change, MSRV) — see
  [`verification-commands.md`](verification-commands.md).

## 6. Say what you didn't verify

State explicitly, in the `tasks.md`/`issues.md` entry and the commit
message, anything you could not check and why — a missing test
environment, a platform you don't have access to, a code path that needs a
specific server version or a real Kerberos/Always-On/NTLM setup this
environment doesn't have. A silent skip reads as "verified" to the next
person; an explicit "not verified: X, because Y" does not. `tasks.md`'s
`#408` (NTLM wire handshake, no NTLM-capable server reachable) and `#413`
(a successful Strict connection, blocked by an ARM/QEMU crash) are the
pattern.

## 7. Land it, defer it, or reject it — and record which

- **Build now**: small, safe, clearly valuable as submitted.
- **Build with modifications**: the underlying idea is right but the diff
  needs rework first (wrong API shape, incomplete tests, needs
  reimplementation against current source, needs combining with another
  PR/issue fixing the same bug).
- **Defer**: legitimate but large, low-urgency, or needs real feature-design
  work discovered only once you started (state the finding, like `tasks.md`'s
  `#275` entry does, rather than leaving a stale "medium effort" estimate).
- **Reject**: security/supply-chain concern, out of scope for this fork's
  stated mission, or superseded by work already done.

Whichever outcome, add or update the entry under the matching section of
`tasks.md` or `issues.md`, in that file's existing voice — terse, cites the
PR/issue number, states what was verified live vs. by inspection, and links
the commit once one exists.
