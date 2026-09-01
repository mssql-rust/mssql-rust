# mssql-rust

A native, asynchronous Rust client for Microsoft SQL Server (TDS) — a
community fork of [Tiberius](https://github.com/prisma/tiberius), created to
prioritize ongoing maintenance and security updates over large rewrites.

[![crates.io](https://img.shields.io/crates/v/mssql.svg)](https://crates.io/crates/mssql)
[![docs.rs](https://docs.rs/mssql/badge.svg)](https://docs.rs/mssql)
[![Cargo tests](https://github.com/mssql-rust/mssql-rust/actions/workflows/test.yml/badge.svg)](https://github.com/mssql-rust/mssql-rust/actions/workflows/test.yml)

Landing page: <https://mssql-rust.github.io/>

## Quickstart

```sh
cargo add mssql
```

See [`mssql/README.md`](mssql/README.md) for a full walkthrough — connecting,
authentication, TLS backends, bulk insert — and
[`mssql/CHANGELOG.md`](mssql/CHANGELOG.md) for what's changed since the
Tiberius fork point.

## What's in this repository

This is a monorepo with two independent subprojects:

| Subproject | Directory | What it is |
| --- | --- | --- |
| The crate | [`mssql/`](mssql/) | The Rust TDS client itself — `cargo build`/`test` from here |
| The site | [`mssql-rust.github.io/`](mssql-rust.github.io/) | Source for the docs/landing site (SvelteKit) — `npm` from here. Published via [`make github-pages`](Makefile) to the separate [`mssql-rust/mssql-rust.github.io`](https://github.com/mssql-rust/mssql-rust.github.io) repo that GitHub Pages actually serves from; see [`spec/monorepo-github-pages/`](spec/monorepo-github-pages/) for why |

They're independent builds that happen to live in one repository for
convenience — a change to one only rarely needs a change to the other.

Everything else supports those two:

- [`AGENTS.md`](AGENTS.md) — operational guidance for anyone, human or AI
  agent, working in this repository; start there before contributing.
- [`mssql-skill/`](mssql-skill/), [`mssql-rust-maintainer-skill/`](mssql-rust-maintainer-skill/) —
  packaged [Claude Code](https://claude.com/claude-code) skills for using or
  maintaining the crate.
- [`spec/`](spec/) — specs stating what must be true, independent of how a
  given change gets there; `mssql/spec/` holds crate-specific ones.
- [`agents/`](agents/) — topic guides referenced from `AGENTS.md`.
- [`Makefile`](Makefile), [`bin/`](bin/) — cross-cutting repository tasks
  that don't belong to either subproject's own tooling (cargo/npm).

## The fork, in one paragraph

`mssql` exists to prioritize ongoing maintenance and security updates ahead
of current SQL Server/TDS protocol versions, via small, focused commits
rather than large rewrites. The full commit history from Tiberius is
preserved. Same dual MIT/Apache-2.0 license as upstream — see
[`mssql/LICENSE-MIT.txt`](mssql/LICENSE-MIT.txt) and
[`mssql/LICENSE-APACHE.txt`](mssql/LICENSE-APACHE.txt). Many thanks to the
Tiberius team and community for the foundation this fork builds on.

## Mirrors

Also mirrored to [GitLab](https://gitlab.com/mssql-rust/mssql-rust) and
[Codeberg](https://codeberg.org/mssql-rust/mssql-rust).
