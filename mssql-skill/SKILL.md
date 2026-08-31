---
name: mssql-skill
description: Explains how to use the `mssql` crate to talk to Microsoft SQL Server from Rust — connecting, choosing a TLS backend or authentication method, TDS wire-protocol basics as they show up in the API, picking feature flags, and where to find a worked example in this repo. Use when someone is writing Rust code against `mssql`, as opposed to maintaining the fork itself (that's `mssql-rust-maintainer-skill`).
---

# Using the mssql crate

This skill is for **people writing Rust code against `mssql`**: connecting to
SQL Server, choosing a TLS backend or authentication method, picking feature
flags, running a query or a bulk insert. It is not the maintainer's guide —
for triaging upstream Tiberius PRs/issues, releasing, or auditing this fork's
policy compliance, `mssql-rust-maintainer-skill` covers that instead.

`mssql` is a native, asynchronous, runtime-independent TDS (Tabular Data
Stream) client for Microsoft SQL Server. It is a maintenance-focused fork of
[Tiberius](https://github.com/prisma/tiberius).

## Where to go for what

| You want | Read |
| --- | --- |
| Core concepts explained (`Client`/`Config`/`Query`/`Row`/`ColumnData`, runtime independence, TLS backends, auth methods) | [`references/concepts.md`](references/concepts.md) |
| The full Cargo feature flag table | [`references/feature-flags.md`](references/feature-flags.md) |
| A worked example in this repo's own code | [`references/examples.md`](references/examples.md) |
| The quickstart, full configuration surface, and encryption/auth prose | the crate's own [`README.md`](../mssql/README.md) |
| The complete, authoritative API reference | [docs.rs/mssql](https://docs.rs/mssql) |
| What changed recently (unreleased features, fixes) | [`CHANGELOG.md`](../mssql/CHANGELOG.md) |

## How to use this skill

- Start from `concepts.md` for "what is a `Config`/`Client`/`Query`" or
  "which TLS backend/auth method should I use" — this is general to the
  crate's design and should stay accurate as the code evolves; verify
  anything version-specific against docs.rs before treating it as final.
- Reach for `feature-flags.md` when the question is "which Cargo feature
  turns X on, and what's its default" — it's read from `Cargo.toml` directly
  rather than repeated prose that can drift out of date.
- Reach for `examples.md` when the question is "show me where this shows up
  in this codebase" — it points at real files under `mssql/examples/` rather
  than repeating code that can go stale. Prefer reading the linked file over
  trusting a remembered snippet.
- If a question turns out to be about fixing a bug in this fork, triaging an
  upstream PR or issue, or releasing a new version, that's the maintainer
  skill's territory — say so rather than guessing at process details.
- If a question is about TDS or SQL Server in general and this crate has no
  special angle on it, answer from general knowledge directly; there's no
  need to force a repo reference where none is relevant.
