# Spec index — single source of truth

This monorepo has two `spec/` locations, because it has two independently
scoped things: a monorepo-wide policy vs. the `mssql` crate's own policies.
Neither is the "real" one — each is canonical for its own scope. Don't copy a
topic between them; link to wherever it actually lives.

| Topic | Applies to | Location |
| --- | --- | --- |
| AI guidance files (`llms.json`/`llms.txt`) | Both `mssql/` and `mssql-rust.github.io/` publish their own copies | [`llms-json-and-llms-txt/index.md`](llms-json-and-llms-txt/index.md) |
| Dependabot | The `mssql` crate's GitHub repo | [`../mssql/spec/dependabot/index.md`](../mssql/spec/dependabot/index.md) |
| Rust MSRV policy (current stable − 2) | The `mssql` crate's own Cargo workspace | [`../mssql/spec/rust-msrv-n-minus-2/index.md`](../mssql/spec/rust-msrv-n-minus-2/index.md) |
| AI release authorization | The `mssql` crate's release process | [`../mssql/spec/ai-release-authorization/index.md`](../mssql/spec/ai-release-authorization/index.md) |
| Monorepo → GitHub Pages export | `mssql-rust.github.io/` publishing | [`monorepo-github-pages/index.md`](monorepo-github-pages/index.md) |
| Node current version | `mssql-rust.github.io/` toolchain | [`node-current-version/index.md`](node-current-version/index.md) |

## Why a topic lives where it does

- **Here** (monorepo root `spec/`): a policy that could apply to *any*
  subproject this monorepo ever grows (today: `mssql/` and
  `mssql-rust.github.io/`). `llms-json-and-llms-txt` is here because both
  subprojects publish their own `llms.json`/`llms.txt`.
- **`mssql/spec/`**: a policy specific to the Rust crate — its Cargo
  workspace, its GitHub repo settings, its CI. `mssql-rust.github.io/` has no
  `spec/` of its own yet; if it needs a policy that doesn't apply to `mssql/`
  (a SvelteKit/Lily convention, say), it gets its own `mssql-rust.github.io/spec/`
  rather than crowding either existing location.

## What "single source of truth" means here

Each topic's `index.md` is authoritative for that topic. Code, CI config, and
docs conform to it — not the other way around. When a spec doc and the actual
repo state disagree, that is a bug in one of the two, not a matter of
interpretation: either the doc is stale (fix the doc, or fix the repo to match
it, whichever is actually correct) or the repo drifted (fix the repo).

A behavioral or policy change should update the spec doc **in the same
change** as the code/config that implements it — not as a follow-up. See
[`AGENTS.md`](../AGENTS.md) for how this fits into the rest of the workflow
(it's lighter-weight here than a full requirement-ID system: this monorepo is
small enough that a handful of named topic docs plus `mssql/tasks.md`'s
build-queue discipline cover it without needing numbered requirements).
