---
name: mssql-rust-maintainer-skill
description: Technical maintenance skill for this fork of Tiberius — triaging an upstream Tiberius PR or issue, verifying a change before claiming it works, and preparing a release. Use when the task is maintaining, fixing, or reviewing this fork itself, as opposed to writing application code against the `mssql` crate (that's `mssql-skill`).
---

# Maintaining mssql (the Tiberius fork)

`mssql` is a fork of [Tiberius](https://github.com/prisma/tiberius) whose
whole reason for existing is ongoing maintenance and security updates — see
[`README.md`](../mssql/README.md#fork-of-tiberius) and [`src/lib.rs`](../mssql/src/lib.rs)'s
crate-level doc comment for the fork's stated goals: prioritize current SQL
Server/TDS versions, favor small focused commits over large rewrites, keep
the same MIT/Apache-2.0 licensing as upstream.

This skill is the **task-oriented layer**: checklists and exact commands.
The judgment calls themselves — what counts as a security risk, when a PR
mixes unrelated concerns, what "verified" actually means here — are best
learned by reading how this fork has already made them, in
[`tasks.md`](../mssql/tasks.md) (upstream PR triage) and
[`issues.md`](../mssql/issues.md) (upstream issue triage). Both are living
build queues, checked off as items land — read the whole file for a given
task, not just the entry that looks closest to what you're doing, since the
surrounding entries show the reasoning style expected.

## Read first, in this order

1. [`README.md`](../mssql/README.md)'s "Fork of Tiberius" section and
   [`src/lib.rs`](../mssql/src/lib.rs)'s top doc comment — why this fork
   exists and what it explicitly does *not* try to be (no connection
   pooling, no query builder, no ORM).
2. [`CONTRIBUTING.md`](../mssql/CONTRIBUTING.md) — the setup, test, and
   code-style commands this skill's `verification-commands.md` distills.
3. [`tasks.md`](../mssql/tasks.md) or [`issues.md`](../mssql/issues.md),
   whichever matches your work — read the section headers (Reject / Build
   now / Build with modifications / Defer / already superseded or resolved)
   and several full entries, not just one.
4. [`CHANGELOG.md`](../mssql/CHANGELOG.md)'s "Unreleased" section — what
   this cycle has already built, so a new PR/issue can be checked against
   what's already landed instead of duplicating it.
5. If the change touches MSRV or dependency policy:
   [`spec/rust-msrv-n-minus-2/index.md`](../mssql/spec/rust-msrv-n-minus-2/index.md)
   and [`spec/dependabot/index.md`](../mssql/spec/dependabot/index.md).

## Which checklist applies

| Your task | Use |
| --- | --- |
| Triaging the next upstream Tiberius PR or issue | [`references/pr-triage-checklist.md`](references/pr-triage-checklist.md) |
| Verifying a claim before writing it down ("this works", "fixes #NNN") | [`references/verification-commands.md`](references/verification-commands.md) |
| Preparing a release (version bump, CHANGELOG, tag) | [`references/release-checklist.md`](references/release-checklist.md) |
| Reporting or handling a security issue | [`CONTRIBUTING.md`](../mssql/CONTRIBUTING.md)'s "Before you start" section and [`README.md`](../mssql/README.md#security) directly — both are already concrete |

## The discipline this skill exists to reinforce

Distilled from how `tasks.md` and `issues.md` actually read, entry by entry:

1. **Read the whole diff, not just the title or description.** Several
   entries in `tasks.md` reject a PR only after reading it in full (e.g.
   `#328`'s SQL-injection-shaped string interpolation, found by reading the
   code, not the PR summary).
2. **Security and supply-chain risk come first.** An unpinned git dependency
   pointed at a random fork (`#132`), a silent default-behavior flip with no
   version bump (`#405`), an ungated proc-macro dependency (`#328`) — all
   rejected on that basis alone, independent of whether the feature itself
   had merit.
3. **Verify live against a real SQL Server when the change touches wire
   protocol or bulk insert**, using `docker/test-server.sh` — this fork's
   own history is that protocol bugs (money/smallmoney encoding, a `DATE`
   TYPE_INFO byte-shift, an RPC header omission) were found only by live
   testing, not unit tests alone.
4. **Say what you didn't verify, and why**, rather than leaving it
   implicit — e.g. `#408`'s NTLM wire handshake, left unverified because no
   NTLM-capable server was reachable; `#413`'s successful Strict connection,
   left unverified because the only ARM-runnable SQL Server 2022 image
   crashes under this host's QEMU emulation.
5. **When a PR mixes unrelated concerns, cherry-pick the relevant hunk**
   rather than merging the whole thing — e.g. taking just the
   `tls_stream.rs`/`Cargo.toml` hunks from `#308`, or just the decimal hunk
   from `#376`, leaving the rest behind.
6. **When several PRs/issues fix the same root cause, consolidate them** and
   take the best parts of each — e.g. the column-name bracket-escaping fix
   combined `#387`'s correct escaping with `#388`'s tests, rather than
   merging either alone.

## Scope discipline

A PR/issue triage pass is not finished by reading the diff and deciding it
looks plausible — it ends with either a landed, tested, live-verified
commit, or an explicit, recorded reason it was rejected or deferred. Say
which outcome you're aiming for before starting, and if new information
mid-review changes that (a "small fix" turns out to need real feature-design
work, as `#275` did), say so and move it to the right tier rather than
quietly finishing a smaller version of it.
