# Releasing the `mssql` crate

There is no automated publish process in this repository (no `publish.yml`,
no `cargo-release` config — checked `mssql/.github/workflows/`). The full
checklist, kept in one place to avoid two copies drifting, is
[`mssql-maintainer-skill/references/release-checklist.md`](../mssql-maintainer-skill/references/release-checklist.md) —
read it before bumping a version. It's honest about what's actually
established (the `CHANGELOG.md` "Unreleased" convention, `Cargo.toml`'s
`version` as the single source of truth) versus what's inferred and
unverified (tagging format, crates.io publishing credentials). If you learn
the answer to one of its open questions, update it there, not here.

The one monorepo-level fact that checklist doesn't cover: the site
(`mssql-rust.github.io/`) has its own independent `version` in
`package.json`, which tracks nothing about the crate — don't bump it as part
of a crate release, and don't expect it to match the crate's version number.
