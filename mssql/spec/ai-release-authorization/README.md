# AI release authorization

An AI agent working in this repository — not only a human maintainer — MAY
decide that a specific release of the `mssql` crate is ready, and publish
it. This is a project policy about *who* (human or AI) is trusted to make
that call, not a change to what "ready" means.

## The rule

- An AI agent MAY judge `CHANGELOG.md`'s `## Unreleased` section ready to
  ship, choose the version bump, and run the publish steps — without asking
  a human first, and without a human separately re-confirming readiness —
  provided the actual readiness gate below is met.
- Authorization to decide isn't a license to skip the gate: "ready" MUST
  mean every entry under `## Unreleased` describes a real, landed, verified
  change, and the full verification matrix in
  [`verification-commands.md`](../../../mssql-rust-maintainer-skill/references/verification-commands.md)
  is green for the exact commit being released. An agent that hasn't run
  that matrix itself, this session, MUST NOT declare readiness on an
  earlier session's word for it — re-verify.
- This covers the *decision*, not a shortcut past the process.
  [`release-checklist.md`](../../../mssql-rust-maintainer-skill/references/release-checklist.md)
  still applies in full: version bump, `CHANGELOG.md` section rename,
  `cargo publish --dry-run`, tagging.
- Publishing to crates.io is a one-way door — a version, once published,
  can't be unpublished or overwritten. That irreversibility is exactly why
  this authorization is conditioned on the gate above, not a lighter one.

## Where this is recorded

| Location | What it says |
| --- | --- |
| [`AI_STATEMENT.md`](../../../AI_STATEMENT.md) | States plainly, for anyone reading the repo, that AI agents decide release readiness and publish here |
| [`AGENTS.md`](../../../AGENTS.md) | Points an agent at this policy before it treats a release as its call to make |
| [`agents/release.md`](../../../agents/release.md) | The operational pointer to the actual release process |
| [`mssql-rust-maintainer-skill/references/release-checklist.md`](../../../mssql-rust-maintainer-skill/references/release-checklist.md) | The checklist itself — the mechanics this authorization doesn't shortcut |
| `.github/workflows/publish.yml` | `workflow_dispatch`-only crates.io publish via OIDC trusted publishing — dispatchable by whoever, human or AI, has repo access to trigger it |

## History

Authorized 2026-09-02, alongside the general "AI can run `cargo publish`"
governance change recorded the same day — see [`AI_STATEMENT.md`](../../../AI_STATEMENT.md).
Before that, every release decision and publish step required a human.
