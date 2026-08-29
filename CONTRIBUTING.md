# Contributing to mssql

Thanks for considering a contribution. `mssql` is a fork of
[Tiberius](https://github.com/prisma/tiberius) focused on ongoing maintenance
and security updates — see the [README](README.md#fork-of-tiberius) for the
full story and the fork's goals. Contributions in that spirit are especially
welcome: dependency and CVE updates, support for current SQL Server and TDS
protocol versions, bug fixes, and small, well-scoped improvements.

## Before you start

- For anything more than a small fix, please open an issue first to discuss
  the approach. It saves rework on both sides.
- Security issues should **not** be filed as public issues — see
  [Security](README.md#security) in the README.
- By contributing, you agree your contribution is licensed under the same
  dual [MIT](LICENSE-MIT.txt) / [Apache-2.0](LICENSE-APACHE.txt) license as
  the rest of the crate.

## Getting set up

You'll need:

- A recent stable Rust toolchain (this repo pins `stable` via
  `rust-toolchain`; see [MSRV](#msrv) below for the minimum it must compile
  on).
- Docker, to run a local SQL Server for integration tests.
- On Linux, the OpenSSL and Kerberos headers if you're touching
  `vendored-openssl` or `integrated-auth-gssapi`: `sudo apt install -y
  openssl libkrb5-dev` (see the [feature flags table](README.md#feature-flags)
  for what each flag needs).

Clone the repo and build:

```sh
git clone https://github.com/mssql-rust/mssql-rust.git
cd mssql-rust
cargo build
```

## Running the tests

Most tests are integration tests that need a live SQL Server. Start one with
Docker Compose (pick whichever version you want to test against):

```sh
DOCKER_BUILDKIT=1 docker compose up -d mssql-2022
```

Set the connection string the tests read (`.envrc` has a ready-to-use default
if you use [direnv](https://direnv.net/)):

```sh
export MSSQL_TEST_CONNECTION_STRING="server=tcp:localhost,1433;user=SA;password=<YourStrong@Passw0rd>;TrustServerCertificate=true"
```

Then run the tests:

```sh
cargo test --features=all
```

Unit tests that don't need a database (`cargo test --lib`) run without any of
the above.

### Feature flags

The crate supports several optional, often mutually-relevant feature
combinations — see the [feature flags table](README.md#feature-flags) for
what each one does. CI exercises this exact matrix, so if you're touching
TLS, date/time, or decimal handling, it's worth checking more than the
default build:

```sh
cargo test --features=all
cargo test --no-default-features
cargo test --no-default-features --features=chrono
cargo test --no-default-features --features=time
cargo test --no-default-features --features=rustls
cargo test --no-default-features --features=vendored-openssl
```

## Code style and checks

Before opening a PR, run what CI runs:

```sh
cargo fmt --check
cargo clippy --features=all -- -D warnings
```

`cargo fmt` (no `--check`) will fix formatting for you. Clippy warnings are
treated as errors in CI, so please fix or (rarely, with a comment explaining
why) `#[allow]` them rather than leaving them for review.

### MSRV

This repo's Minimum Supported Rust Version policy is documented in
[`spec/rust-msrv-n-minus-2/index.md`](spec/rust-msrv-n-minus-2/index.md):
current stable Rust minus two minor versions, recorded as `rust-version` in
`Cargo.toml`. CI's `msrv` job compiles the workspace on exactly that pinned
toolchain — if you use a language or standard-library feature newer than the
MSRV, that job will fail. Check locally with:

```sh
cargo +<msrv-version> check --all-targets --workspace
```

## Commits and pull requests

- Favor small, focused commits over large, sweeping ones — this mirrors the
  fork's own stated priorities and makes review and future bisecting easier.
- A short, imperative commit subject works well, optionally in the loose
  `type: subject` style already used in this repo's history (`fix:`,
  `feat:`, `chore:`, `style:`, `test:`, `ci:`, `docs:`, `refactor:`). Not a
  hard requirement, just a helpful convention.
- Update [`CHANGELOG.md`](CHANGELOG.md) for user-facing changes.
- Make sure `cargo fmt --check`, `cargo clippy --features=all -- -D
  warnings`, and the relevant `cargo test` feature combinations pass before
  requesting review.
- CI runs clippy, rustfmt, the MSRV check, and the full test matrix (Linux,
  Windows, macOS) against several SQL Server versions on every PR.

## Reporting bugs

Please include: the `mssql` version, the feature flags in use, the SQL
Server version and platform you're targeting, and a minimal reproduction if
possible. If the issue involves a protocol-level detail, a packet capture or
reference to the [MS-TDS specification](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-tds/)
is very helpful.

## Thanks

Thanks again to the Tiberius maintainers and contributors whose work this
fork is built on, and to everyone who contributes here.
