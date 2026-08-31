# CLAUDE.md

See [`AGENTS.md`](AGENTS.md) — it's the actual guidance and applies to every
agent equally, Claude Code included. This file only adds the handful of
things specific to working here *as* Claude Code.

## Skills

This repo ships two project skills — invoke them explicitly rather than
re-deriving their content by reading the whole tree each time:

- `/mssql-skill` when writing or reviewing code that *uses* the crate
  (connecting, choosing a TLS backend or auth method, picking feature flags).
- `/mssql-rust-maintainer-skill` when triaging an upstream Tiberius PR/issue,
  verifying a change, or preparing a release.

## Local conventions worth knowing before you start

- The monorepo root (`~/git/mssql-rust/mssql-rust`) is **not itself** what
  most work touches — `cd` into `mssql/` for anything `cargo`, or
  `mssql-rust.github.io/` for anything `npm`. Each has its own toolchain and
  its own `spec/`-adjacent conventions; see [`AGENTS.md`](AGENTS.md).
- `mssql/Cargo.lock` is gitignored (a library crate, not a binary) — don't
  be surprised it doesn't show up in `git status` after a build.
- The site's Node version is pinned in `.tool-versions`; if the ambient
  shell's `node -v` looks older, use `mise exec -- npm ...` rather than
  bumping anything.
- Before publishing an `llms.json`/`llms.txt` change, re-read
  [`spec/llms-json-and-llms-txt/index.md`](spec/llms-json-and-llms-txt/index.md) —
  it documents a real mistake (a verbatim copy whose self-reference link
  pointed at the wrong domain) worth not repeating.
