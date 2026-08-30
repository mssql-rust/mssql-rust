# Triaging an upstream Tiberius PR or issue

Read this before picking the next thing to build, or when asked to look at a
specific upstream [prisma/tiberius](https://github.com/prisma/tiberius) PR
or issue.

This fork's actual practice lives in two documents — this file summarizes
the discipline; they are the record:

- [`../mssql/tasks.md`](../mssql/tasks.md) — PR triage, sectioned Reject /
  Build now / Build with modifications / Defer / Already superseded, with a
  suggested build order.
- [`../mssql/issues.md`](../mssql/issues.md) — issue triage, same sections,
  security-first.

## The loop

1. **Read the whole diff** (`gh pr diff <N> --repo prisma/tiberius`) or the
   whole issue thread (`gh issue view <N> --repo prisma/tiberius
   --comments`) — not just the title or description. Several rejections in
   `tasks.md` exist only because someone read the actual diff (a
   SQL-injection-shaped string interpolation in #328's TVP support wasn't
   mentioned in its description).
2. **Security and supply-chain risk first.** An unpinned dependency on a
   random fork, a silent default-behavior flip with no version bump, an
   ungated proc-macro dependency — reject on that basis alone, independent
   of whether the underlying feature has merit. File it under Reject with
   the specific reason, not a vague "looks risky."
3. **Check what this fork has already done differently.** Read
   `CHANGELOG.md`'s "Unreleased" section and search the git log — this fork
   has its own naming (`mssql`/`MSSQL_TEST_*`, not `tiberius`/`TIBERIUS_*`),
   its own already-merged fixes, and sometimes a PR duplicates or is
   superseded by work already done. Say so and move it to "Already
   superseded" rather than re-doing it.
4. **Decide: Build now / Build with modifications / Defer / Reject.**
   - *Build now*: small, safe, real value, nothing to rework.
   - *Build with modifications*: real value, but needs rework — reimplement
     against this fork's current source rather than patching stale upstream
     code, cherry-pick the relevant hunk out of a multi-concern diff, or
     combine several overlapping PRs' best parts (bracket-escaping
     consolidated three PRs' worth of fixes into one, for example).
   - *Defer*: not wrong, just low priority, already covered elsewhere, or
     blocked on something external (e.g. the PR's author marking it ready
     for review) — **name the condition that would move it to Build**, so a
     later pass can check whether that condition changed rather than
     guessing.
   - *Reject*: name the specific defect. "Rejected" entries stay in the
     document permanently, unchecked — they are not TODOs.
5. **Implement, then verify live wherever the change touches wire protocol
   or bulk insert** — see [`rust.md`](rust.md)'s Testing section. Say
   explicitly what you verified and how (live server / unit tests only /
   inspection only, and why) in both the commit message and the `tasks.md`/
   `issues.md` entry.
6. **Check it off** (`[x]`) with a one-line "Done: `<commit sha>`" note,
   in-place — don't delete or move finished entries out of their section
   header; the document's value is the full record, reject and defer
   entries included.
7. **Add a `CHANGELOG.md` "Unreleased" entry.**

## Revisiting a Deferred item

Before building something marked Defer, check whether the stated condition
actually changed (e.g. `gh pr view <N> --repo prisma/tiberius --json
isDraft,state,updatedAt` for "revisit once marked ready for review"). If it
hasn't changed, building it anyway is a legitimate call — the original
deferral reasoning might not still apply, or the approach might be sound
even if administratively unchanged — but say explicitly that the condition
is unmet and why you're proceeding regardless, rather than silently treating
Defer as Build now.
