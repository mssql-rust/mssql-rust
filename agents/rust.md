# Editing `mssql/`

Read this before touching any file under `mssql/src/`, `mssql/tests/`,
`mssql/examples/`, or `mssql/Cargo.toml`.

## The non-negotiables

- **MSRV**: current stable minus two minor versions (currently `1.96`; see
  [`../mssql/spec/rust-msrv-n-minus-2/index.md`](../mssql/spec/rust-msrv-n-minus-2/index.md)).
  Don't use a language/stdlib feature stabilized after the MSRV. `cargo
  +1.96 check --all-targets` is the ground truth, not "it compiles on my
  machine."
- **`-D warnings`**: CI runs `cargo clippy` with warnings denied, under both
  default features and `--features all`. Run both locally before calling
  something done.
- **License header / attribution stays**: MIT/Apache-2.0 dual license,
  unchanged from Tiberius. `src/lib.rs`'s top doc comment and `README.md`'s
  "Fork of Tiberius" section both thank the Tiberius team explicitly — don't
  edit those out incidentally while touching something else nearby.

## Feature-gating a new type or API

Every optional capability in this crate is additive and off by default
(check `[features]` in `Cargo.toml` — `default = ["tds73", "winauth",
"native-tls"]`, everything else opt-in). When adding something
feature-gated:

1. Add the feature to `Cargo.toml`'s `[features]`, with a comment explaining
   *why* it's separate (see `sspi-rs`'s or `rustls-webpki-roots`'s comments
   for the expected level of detail) — not just what it does.
2. Add it to the `all` feature group too, so CI's `--features all` job
   actually exercises it.
3. If it needs a feature-gated example or test target, add a `[[example]]`
   or `[[test]]` stanza with `required-features = [...]` and a comment
   explaining why the target can't just be skipped-at-runtime (see the
   `webpki_roots` and `serde_json` example stanzas) — otherwise `cargo check
   --all-targets` under default features fails to compile it.
4. Document the flag in `README.md`'s feature-flag table (keep it
   alphabetical-ish and matching the existing row format) and, if it's the
   kind of thing a user picks between (a TLS backend, an auth method), in
   `../mssql-skill/references/feature-flags.md` too.
5. Update `llms.json`'s `feature_flags` map and `llms.txt`'s relevant
   section in **both** `mssql/` and `mssql-rust.github.io/static/` — see
   [`../spec/llms-json-and-llms-txt/index.md`](../spec/llms-json-and-llms-txt/index.md)
   for why those two copies aren't identical.
6. Add a `CHANGELOG.md` "Unreleased" → "Added" entry.

## Testing

- `cargo test --lib` — unit tests, no network needed, both default and
  `--features all`.
- `cargo test --test <name> --features <its required-features>` — the
  feature-gated integration test binaries declared as `[[test]]` stanzas
  (`serde`, `named-instance-*`).
- `cargo test --test query --features all` and friends need a **live SQL
  Server**. Use [`docker/test-server.sh`](../mssql/docker/test-server.sh) to
  bring one up locally (podman or docker) before running these — don't claim
  a wire-protocol or bulk-insert change is verified without having actually
  run it against a live server, even if the unit tests pass. If no live
  server is reachable in your environment, say so explicitly rather than
  silently only running the unit tests.

## Verifying a fix against upstream

If the change originates from an upstream Tiberius PR or issue (most do —
see [`pr-triage.md`](pr-triage.md)), read that PR/issue's **full diff**, not
just its description, before deciding what to port. This fork's own history
(`tasks.md`) has repeated examples of a PR's stated purpose being fine but
its actual diff mixing in something that shouldn't land (an unrelated
dependency bump, a silent behavior flip) — cherry-pick the relevant hunk
rather than merging wholesale when that happens.
