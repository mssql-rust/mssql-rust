# Verifying a change

The actual commands this project uses, taken from
[`CONTRIBUTING.md`](../../mssql/CONTRIBUTING.md) and
[`.github/workflows/test.yml`](../../.github/workflows/test.yml). Run
these — or point at the CI job that already runs them — before writing
"fixes #NNN" or checking off a `tasks.md`/`issues.md` entry. All commands
below are run from `mssql/` (the crate root, which is also its own Cargo
workspace — see `spec/rust-msrv-n-minus-2/index.md`).

## Build and unit tests (no database needed)

```sh
cargo build
cargo test --lib
```

## Style and lints (what CI's `clippy`/`format` jobs run)

```sh
cargo fmt --check          # cargo fmt (no --check) fixes it for you
cargo clippy --features=all -- -D warnings
```

Clippy warnings are errors in CI — fix them or add a commented `#[allow]`,
don't leave them for review.

## Benchmarks and coverage (not in CI; run by hand when relevant)

```sh
cd mssql
cargo bench --bench offline                       # criterion, no server
cargo bench --bench live                          # needs MSSQL_TEST_CONNECTION_STRING
cargo tarpaulin                                   # coverage: unit + tls_local
cargo tarpaulin --config tarpaulin-live.toml      # coverage incl. the live suite
```

For a performance-sensitive change, run `-- --save-baseline before` on the
base commit and `-- --baseline before` on the change. Coverage baseline
(2026-11-17): 46.6% offline and 71.9% live, on Azure SQL Edge.

## MSRV check (what CI's `msrv` job runs)

```sh
cargo +1.98 check --all-targets --workspace
```

`1.98` is the value currently pinned in `Cargo.toml`'s
`[workspace.package] rust-version` — read that file for the current number
rather than trusting this one if it's been a while; see
[`spec/rust-msrv-n-minus-2/index.md`](../../mssql/spec/rust-msrv-n-minus-2/index.md)
for the policy (current stable minus two minor releases) and the steps to
follow when it needs to move.

## Live-database integration tests

Most tests are integration tests that need a real SQL Server. Start one
with this fork's own helper (podman or docker, picks whichever is
installed):

```sh
./docker/test-server.sh up      # build, start, wait until it accepts connections
export MSSQL_TEST_CONNECTION_STRING='server=tcp:localhost,1433;user=SA;password=<YourStrong@Passw0rd>;IntegratedSecurity=true;TrustServerCertificate=true'
cargo test
./docker/test-server.sh down    # stop and remove when done
```

(`./docker/test-server.sh logs` follows the container log if it doesn't
come up.) Alternatively, `docker-compose.yml` at the crate root defines
named services per SQL Server version — `DOCKER_BUILDKIT=1 docker compose
up -d mssql-2022` (or `mssql-2019`, `mssql-2017`) — which is what CI's Linux
and macOS jobs use.

## Testing against SQL Server 2022/2025 on Apple Silicon

Only Azure SQL Edge runs natively on arm64. The 2022 and 2025 images are
x86-64-only, segfault under podman/QEMU, and run fine under **Docker Desktop
with Settings → General → "Use Rosetta for x86_64/amd64 emulation"** on:

```sh
docker --context desktop-linux run -d --name mssql-2025 --platform linux/amd64 \
  -e ACCEPT_EULA=Y -e 'MSSQL_SA_PASSWORD=<YourStrong@Passw0rd>' -p 1436:1433 \
  mcr.microsoft.com/mssql/server:2025-latest
export MSSQL_TEST_CONNECTION_STRING='server=tcp:localhost,1436;user=SA;password=<YourStrong@Passw0rd>;TrustServerCertificate=true'
cargo test --no-default-features --features rustls,tds73
```

A real TDS 8.0 Strict login (`tests/strict.rs`, opt-in through
`MSSQL_TEST_STRICT_CONNECTION_STRING=...;encrypt=strict;TrustServerCertificateCA=<cert>`)
needs the server started with `--hostname localhost`, plus an `mssql.conf` with
`tlscert`/`tlskey` (CN/SAN `localhost`, `keyUsage=digitalSignature,keyEncipherment`,
`extendedKeyUsage=serverAuth`, `CA:FALSE`) and `tlsprotocols = 1.2,1.3`.
Verified on SQL Server 2025. SQL Server 2022 on Linux refused Strict here
(error 17821) with the same certificate.

## The feature-flag matrix CI actually exercises

Touching TLS, date/time, or decimal handling is not fully checked by the
default build alone. CI's `cargo-test-linux` job runs the full cross
product of SQL Server versions (2017, 2019, 2022, azure-sql-edge) against:

```sh
cargo test --features=all
cargo test --no-default-features
cargo test --no-default-features --features=jiff
cargo test --no-default-features --features=time
cargo test --no-default-features --features=rustls
cargo test --no-default-features --features=rustls-webpki-roots
cargo test --no-default-features --features=vendored-openssl
```

Windows CI additionally covers `--no-default-features
--features=rustls,winauth` and `--no-default-features
--features=vendored-openssl,winauth`; macOS CI covers a broad
`--no-default-features --features=rustls,jiff,time,tds73,sql-browser-*,
integrated-auth-gssapi,rust_decimal,bigdecimal` combination plus
`--no-default-features --features=vendored-openssl`. Run the legs relevant
to what changed, not necessarily all of them — but if the change touches a
shared code path (e.g. `Config` parsing, the TLS-backend-selection logic),
run at least one leg per TLS backend.

If the `serde` feature is involved, its tests are gated behind
`required-features = ["serde"]` in `Cargo.toml`'s `[[test]]` entry for
`tests/serde.rs` — `cargo test --features=all` or
`cargo test --features=serde` both cover it; plain `cargo test` does not.

## Security-relevant checks

- `cargo deny check` (from `mssql/`): advisories, licenses, banned and
  unknown sources, per [`deny.toml`](../../mssql/deny.toml). CI's
  `cargo-deny` job runs the same thing. A new dependency with a license
  outside `deny.toml`'s allow list fails it, which is intended.
- `cargo audit` — advisory-database check. Documented, accepted exceptions
  live in [`.cargo/audit.toml`](../../mssql/.cargo/audit.toml), each with a
  dated justification; re-check an ignored advisory whenever a dependency
  bump might have resolved it, and remove the entry the moment a real fix
  lands upstream.
- Dependabot is enabled at the repo level for both security updates and
  scheduled version-update PRs (weekly, per
  [`.github/dependabot.yml`](../../.github/dependabot.yml)) — see
  [`spec/dependabot/index.md`](../../mssql/spec/dependabot/index.md) for the
  policy statement.
- `.github/workflows/pr-code-security.yml` runs Gitleaks (secret detection)
  and CodeQL (`languages: rust`) on every PR against `main`.

## The general rule

If you can't point at the command output, the test file, or the CI run that
backs a sentence, don't write the sentence yet — run it, or write down what
you actually know, including "not verified, because Y" (see
[`pr-triage-checklist.md`](pr-triage-checklist.md) step 6). Both
`tasks.md` and `issues.md` already model this: entries there routinely
state a specific test count and feature combination ("311 tests over
rustls", "clippy -D warnings under both default and all features") rather
than an unqualified "verified".
