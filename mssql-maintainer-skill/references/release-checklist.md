# Preparing a release

**There is no automated publish process in this repository as of this
writing.** A search of `mssql/.github/workflows/` turns up `test.yml`
(build/test/lint/MSRV) and `pr-code-security.yml` (Gitleaks/CodeQL) only —
no `publish.yml`, no `release.yml`, no `cargo-release` config file anywhere
in the tree. Releasing to crates.io today means a maintainer running
`cargo publish` by hand, from a clean checkout, after the steps below — not
something a CI job does automatically on a tag push. If that has changed
since this was written, trust the actual workflow files over this
document, and update this checklist to match.

The steps below are inferred from what the repository's own files actually
track — `CHANGELOG.md`'s structure and `Cargo.toml`'s `version` field — not
from a documented release runbook, because none exists yet.

## What the repo's own files establish

- `mssql/Cargo.toml`'s `[package] version` is the single source of truth for
  the published version (currently `0.12.3` as of this writing — check the
  live file, don't trust this number).
- `mssql/CHANGELOG.md` accumulates entries under an `## Unreleased` heading
  (with `### Added` / `### Changed` / `### Fixed` / `### Security`
  subsections) as they land, per
  [`CONTRIBUTING.md`](../../mssql/CONTRIBUTING.md)'s "Update CHANGELOG.md
  for user-facing changes" instruction. Below `## Unreleased`, the file
  keeps a full history of dated `## Version X.Y.Z` sections going back
  through Tiberius's own changelog — that's the format a new release's
  section should match.
- The crate's version and its `CHANGELOG.md` entry are two separate facts
  that must agree — `spec/rust-msrv-n-minus-2/index.md`'s own "single source
  of truth" framing for `rust-version` is a useful model: don't let a
  version bump land without the corresponding changelog section, or vice
  versa.

## A sensible sequence, given that

1. **Confirm `Unreleased` is actually ready to ship.** Read it end to end —
   every entry should describe a real, landed, verified change (see
   [`verification-commands.md`](verification-commands.md) and
   [`pr-triage-checklist.md`](pr-triage-checklist.md)), not an
   in-progress or speculative one.
2. **Run the full verification matrix** from
   [`verification-commands.md`](verification-commands.md) against the exact
   commit you intend to release: build, `cargo test --lib`, `cargo fmt
   --check`, `cargo clippy --features=all -- -D warnings`, the MSRV check,
   and at least the `--features=all` and default-feature legs of the
   live-database suite via `docker/test-server.sh`.
3. **Decide the version number** per semver, informed by what's actually in
   `Unreleased` — a breaking API change (this fork's changelog marks these
   `BREAKING:` in past entries) needs a major/minor bump per the crate's
   pre-1.0/post-1.0 status at the time; an additive feature or fix-only
   release does not.
4. **Turn `## Unreleased` into a dated version section**: rename the
   heading to `## Version X.Y.Z` (matching the existing history's format)
   and start a fresh, empty `## Unreleased` above it for the next cycle.
5. **Bump `version` in `mssql/Cargo.toml`** to match. `Cargo.lock` will pick
   up the change on the next build; check it's committed alongside.
6. **Tag the release** — no tagging convention is documented in this
   repository; the crate's history (`## Version 0.12.3`, etc.) suggests a
   plain version string, but confirm against the actual git tags already
   pushed (`git tag --list`) before inventing a new format.
7. **Publish**: `cargo publish` from a clean checkout of the tagged commit,
   after a `cargo publish --dry-run` to catch packaging issues first. No
   crates.io API token management, 2FA, or trusted-publishing setup is
   documented here — set that up (or confirm it's already set up) before
   relying on this step working non-interactively.
8. **Push the tag and the version-bump commit**, per this repo's standing
   "ask first before pushing" convention if one is in effect for your
   environment.

## What's honestly unverified about this checklist

- Whether releases have historically been tagged at all, and if so, in what
  format — check `git tag --list` rather than assuming.
- Whether crates.io publishing credentials/trusted publishing are configured
  for this repository — nothing in `.github/workflows/` references
  `cargo publish` or a crates.io token secret.
- Whether GitHub Releases (as opposed to just crates.io + a CHANGELOG
  section) are part of the intended process — no workflow or template for
  one exists in this tree today.

If any of these turn out to be handled elsewhere (a maintainer's local
script, a process that lives outside this repository), update this file to
say so concretely, with a pointer — don't let this checklist keep asserting
"no process exists" once one does.
