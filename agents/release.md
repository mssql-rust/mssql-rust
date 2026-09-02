# Releasing the `mssql` crate

An AI agent may decide a specific release is ready and publish it, not just
prepare one for a human to publish — see
[`spec/ai-release-authorization/index.md`](../mssql/spec/ai-release-authorization/index.md)
for the rule and the readiness gate it's conditioned on.

Publishing itself runs via `.github/workflows/publish.yml`
(`workflow_dispatch`-only, crates.io trusted publishing via OIDC — no
`CARGO_REGISTRY_TOKEN` secret to manage). The full checklist, kept in one
place to avoid two copies drifting, is
[`mssql-rust-maintainer-skill/references/release-checklist.md`](../mssql-rust-maintainer-skill/references/release-checklist.md) —
read it before bumping a version. It's honest about what's actually
established (the `CHANGELOG.md` "Unreleased" convention, `Cargo.toml`'s
`version` as the single source of truth) versus what's inferred and
unverified (tagging format). If you learn the answer to one of its open
questions, update it there, not here.

The one monorepo-level fact that checklist doesn't cover: the site
(`mssql-rust.github.io/`) has its own independent `version` in
`package.json`, which tracks nothing about the crate — don't bump it as part
of a crate release, and don't expect it to match the crate's version number.
