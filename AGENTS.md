# Working in this repository

Operational guidance for contributors, human and agent. This file says **how
to work**; [`spec/`](spec/index.md) says **what must be true**. When they
seem to conflict, the spec wins and this file is wrong — fix this file.

Read this file first, then the one topic file in [`agents/`](agents/) that
covers what you are about to touch.

## What this is

A two-subproject monorepo, each with its own git history preserved (merged in
via `git filter-repo`, not `git subtree`):

| Subproject | Directory | What it is |
| --- | --- | --- |
| The crate | [`mssql/`](mssql/) | A Rust TDS client for Microsoft SQL Server — a maintenance-focused fork of [Tiberius](https://github.com/prisma/tiberius) |
| The site | [`mssql-rust.github.io/`](mssql-rust.github.io/) | Its SvelteKit docs/landing site, deployed to GitHub Pages |

They are independent builds (`mssql/` is `cargo`, the site is `npm`) that
happen to live in one repo for convenience. A change to one only rarely needs
a change to the other — see [`agents/site.md`](agents/site.md) for the cases
that do (a new feature worth a landing-page mention, a crate-level
`llms.json`/`llms.txt` change that the site's copy should mirror in
site-appropriate form — see [`spec/llms-json-and-llms-txt/index.md`](spec/llms-json-and-llms-txt/index.md)).

## Topic guides

| Guide | Read it when you are |
| --- | --- |
| [rust.md](agents/rust.md) | editing any `mssql/src/**/*.rs`, its `Cargo.toml`, or its tests/examples |
| [pr-triage.md](agents/pr-triage.md) | triaging an upstream Tiberius PR or issue, or picking the next thing to build |
| [release.md](agents/release.md) | bumping the crate version or touching `CHANGELOG.md` |
| [site.md](agents/site.md) | editing anything under `mssql-rust.github.io/` |

## The fork, in one paragraph

`mssql` is a fork of Tiberius created to prioritize ongoing maintenance and
security updates ahead of current MSSQL/TDS versions, via small, focused
commits rather than large rewrites. Every fork-facing surface (crate-level
`README.md`, `src/lib.rs`'s top doc comment, `LICENSE-*.txt`) thanks the
Tiberius team and states this plainly. The dual license (MIT/Apache-2.0) is
unchanged from upstream — never relicense. This applies to `mssql/` only;
the site has no upstream to credit.

## Where "what to build next" comes from

This project does not track work as freeform tickets. Two documents, both in
`mssql/`, are the actual work queues, each covering a different upstream
source:

- **[`tasks.md`](mssql/tasks.md)** — triage of upstream Tiberius **pull
  requests**. Sectioned Reject / Build now / Build with modifications /
  Defer / Already superseded, with a suggested build order at the bottom.
- **[`issues.md`](mssql/issues.md)** — triage of upstream Tiberius **issues**,
  same discipline, security-first.

Both are living documents: check an item off `[x]` with a one-line "Done:
`<commit>`" note when it lands; don't delete finished entries (they're the
record of what was actually verified, and how). See
[`agents/pr-triage.md`](agents/pr-triage.md) for the full discipline before
triaging something new.

## Verification discipline

State what you verified and how, in the commit message — "verified live
against a container" and "verified by inspection, no live server available
here" are both fine; silence about which one happened is not. See
[`agents/rust.md`](agents/rust.md) for the actual commands
(`cargo test`/`clippy`/`docker/test-server.sh`).

## Release authorization

An AI agent may decide a specific release of the `mssql` crate is ready and
publish it — not only prepare one for a human to publish. See
[`mssql/spec/ai-release-authorization/index.md`](mssql/spec/ai-release-authorization/index.md)
for the actual rule and the readiness gate it's conditioned on, and
[`agents/release.md`](agents/release.md) for the mechanics.

## AI-agent skills

Two skill folders at the monorepo root give a task-scoped agent (this
session or another) a faster on-ramp than reading everything above:

- [`mssql-skill/`](mssql-skill/) — using the crate: concepts, feature flags,
  which example to reach for.
- [`mssql-rust-maintainer-skill/`](mssql-rust-maintainer-skill/) — maintaining
  the fork: PR/issue triage, verification commands, release steps.

Load the one that matches the task; this file and the `agents/` guides are
still the source of truth if a skill and this file ever disagree.

## What not to do without asking

- Don't force-push to any of the `origin` remotes (GitHub/Codeberg/GitLab,
  all `mssql-rust/mssql-rust`) or to the `pages` remote
  (`mssql-rust/mssql-rust.github.io`) — a plain push is routine (feature
  branches, tags), but a force-push can silently roll back history or, for
  `pages`, the live site; if a plain push is rejected as non-fast-forward,
  that's the safety mechanism working, not something to override without
  understanding why.
- Don't bump a dependency's version, or the crate's own version, as a side
  effect of an unrelated change.
- Don't relicense, remove the Tiberius attribution, or change the MSRV
  policy's rule (raising the tracked *value* to follow the rule is routine
  and does not need to ask — see [`spec/`](mssql/spec/rust-msrv-n-minus-2/index.md)).
