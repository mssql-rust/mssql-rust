Here is a **prioritized, actionable catalog** of quality-improvement tasks AI can own in this repository. Tasks are grouped by objective, sized (S/M/L), and tagged with the file/area they touch. Each task references the project's own instructions (`AGENTS.md`, `spec/`, `CONTRIBUTING.md`) so the AI can validate compliance as it works.

---

## 🚀 1. New Visitors, Promotion & Upskilling

### 1.1 Landing Page & First Impressions

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-001** | Add **"Why this fork?"** section to site home | S | `mssql-rust.github.io/src/routes/+page.svelte` | One-card summary: Tiberius origin, maintenance-first mission, security emphasis, link to `mssql/README.md#fork-of-tiberius` |
| **S-002** | Add **interactive "Try it"** code block with copy button | S | `mssql-rust.github.io/src/routes/+page.svelte` | Live `cargo add mssql` + minimal connection snippet from `mssql/README.md`, using `ClipboardCopyButton.svelte` |
| **S-003** | Add **feature-compatibility matrix** to site | M | `mssql-rust.github.io/src/routes/+page.svelte` | Table: SQL Server 2005→2025 / Azure SQL, TDS versions, encryption levels, feature flags. Source: `mssql/README.md` |
| **S-004** | Add **Quickstart video** embed or link | S | `mssql-rust.github.io/src/routes/+page.svelte` | 2–3 min screencast: `cargo add`, Docker SQL Server, first query. Host on YouTube/asciinema, link from site |
| **M-001** | Add **"Getting Started" tutorial page** | M | `mssql-rust.github.io/src/routes/getting-started/+page.svelte` | Step-by-step: install Rust, Docker SQL Server, first connection, first query, first bulk insert. Reuse `mssql/examples/tokio.rs` |
| **M-002** | Add **Comparison page: mssql vs Tiberius vs ODBC** | M | `mssql-rust.github.io/src/routes/compare/+page.svelte` | Feature table, maintenance model, performance notes. Link to `mssql/README.md#fork-of-tiberius` |
| **L-001** | **Blog post series: "Migrating from Tiberius to mssql"** | L | `mssql-rust.github.io/src/routes/blog/` | 3-part: migration steps, breaking changes (if any), feature parity. Publish via GitHub Pages |

### 1.2 Documentation & Discoverability

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-005** | Add **FAQ section** to site | S | `mssql-rust.github.io/src/routes/faq/+page.svelte` | Answer top 10 questions from `mssql/issues.md` (e.g., connection pooling, query building, ORM) with links to relevant crates (bb8, sqlx) |
| **S-006** | Add **feature flag reference** to site | S | `mssql-rust.github.io/src/routes/docs/feature-flags/+page.svelte` | Table from `mssql/README.md#feature-flags`, with usage examples |
| **S-007** | Add **examples gallery** to site | M | `mssql-rust.github.io/src/routes/examples/+page.svelte` | List all `mssql/examples/*.rs` with descriptions, copyable snippets, live links to docs.rs for types used |
| **S-008** | Add **contribution guide** to site | S | `mssql-rust.github.io/src/routes/contributing/+page.svelte` | Mirror `mssql/CONTRIBUTING.md` in Markdown, rendered via SvelteKit |
| **M-003** | Add **API reference search** to site | M | `mssql-rust.github.io/src/routes/docs/+page.svelte` | Embed docs.rs search or link prominently. Add Algolia DocSearch if eligible |
| **M-004** | Create **interactive connection string builder** | M | `mssql-rust.github.io/src/routes/tools/connection-string/+page.svelte` | Form fields for host, port, auth, TLS, etc. Generate ADO.NET/JDBC strings. Validate with `connection-string` crate parser |
| **L-002** | **Translate README to Spanish/Chinese** | L | `mssql/README.es.md`, `mssql/README.zh.md` | Human-reviewed translations of `mssql/README.md`. Link from site footer |

### 1.3 Community & Promotion

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-009** | Add **Discord/Matrix community links** | S | `mssql-rust.github.io/src/routes/+page.svelte`, `mssql/README.md` | Links to community chat (create if needed). Update `mssql/README.md` footer |
| **S-010** | Add **sponsorship/funding info** | S | `mssql-rust.github.io/src/routes/+page.svelte`, `.github/FUNDING.yml` | GitHub Sponsors link, Open Collective. Add funding badge to README |
| **S-011** | Add **release RSS feed** | S | `mssql-rust.github.io/src/routes/releases/+page.svelte` | Embed GitHub releases RSS or generate from CHANGELOG |
| **S-012** | Add **newsletter signup** | S | `mssql-rust.github.io/src/routes/+page.svelte` | Mailchimp/Buttondown embed for release announcements |
| **M-005** | Create **GitHub Discussions templates** | M | `.github/DISCUSSIONS.md` | Templates for: Help, Feature Request, Bug Report, Show & Tell. Link from site |
| **M-006** | Create **Twitter/X account** for project | M | N/A | @mssql_rust handle, auto-tweet releases from GitHub Actions |
| **L-003** | **Write guest blog posts** | L | External (Dev.to, Medium, Rust Blog) | "Why we forked Tiberius", "Secure SQL Server connections in Rust", "Bulk insert performance in mssql" |

---

## 🔧 2. Engineering & Coding

### 2.1 Code Health

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-013** | Fix all `clippy -- -D warnings` issues | S | `mssql/src/**/*.rs` | Zero warnings with `--features=all`. Check `mssql/CONTRIBUTING.md#code-style-and-checks` |
| **S-014** | Fix all `cargo fmt --check` issues | S | `mssql/src/**/*.rs` | `cargo fmt --check` passes cleanly |
| **S-015** | Remove all `todo!()`/`unimplemented!()` | S | `mssql/src/**/*.rs` | Replace with proper errors or implementations. Audit via `grep -r` |
| **S-016** | Remove all `unwrap()` in library code | S | `mssql/src/**/*.rs` | Replace with proper error handling. Exclude tests/benchmarks |
| **S-017** | Remove all `expect()` with generic messages | S | `mssql/src/**/*.rs` | Replace with descriptive error messages or proper error types |
| **S-018** | Add `#[derive(Debug)]` to all public types | S | `mssql/src/**/*.rs` | All `pub struct`/`pub enum` have `Debug` derives. Audit via `cargo test --doc` |
| **S-019** | Add `#[derive(Clone)]` where appropriate | S | `mssql/src/**/*.rs` | Types that should be `Clone` have derives. Validate with `cargo clippy -- -D clippy::derive_clone_without_copy` |
| **S-020** | Add `#[derive(PartialEq, Eq, Hash)]` where appropriate | S | `mssql/src/**/*.rs` | Types that should be comparable have derives. Validate semantics |
| **M-007** | Replace `String` with `Zeroizing<String>` for all credentials | M | `mssql/src/**/*.rs` | Passwords, tokens, secrets use `Zeroizing<String>`. Check `AI_STATEMENT.md` for pattern |
| **M-008** | Add `#[must_use]` to all result-returning methods | M | `mssql/src/**/*.rs` | Methods returning `Result<T, E>` where ignoring is likely a bug |
| **M-009** | Add `#[non_exhaustive]` to all public enums | M | `mssql/src/**/*.rs` | All `pub enum` have `#[non_exhaustive]` for future compatibility |
| **M-010** | Add `// SAFETY:` comments to all `unsafe` blocks | M | `mssql/src/**/*.rs` | Every `unsafe {}` has justification. Audit via `cargo +nightly audit unsafe` |
| **M-011** | Migrate to `std::error::Error` for all error types | M | `mssql/src/error.rs` | All error types implement `std::error::Error`. Use `thiserror` where appropriate |
| **L-004** | Refactor `Client` to use builder pattern consistently | L | `mssql/src/client/*.rs` | Replace setters with chainable builder. Maintain backward compatibility |
| **L-005** | Add async trait impls for `AsyncRead`/`AsyncWrite` | L | `mssql/src/**/*.rs` | Allow any async I/O implementation, not just Tokio/smol |

### 2.2 API Improvements

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-021** | Add `Row::get<T>(&str)` convenience method | S | `mssql/src/row.rs` | Lookup by column name with type inference. Already partially exists as `QueryIdx for &str` |
| **S-022** | Add `Config::from_env()` constructor | S | `mssql/src/client/config.rs` | Read connection string from `MSSQL_CONNECTION_STRING` env var |
| **S-023** | Add `Config::builder()` method (if missing) | S | `mssql/src/client/config.rs` | Already exists per `mssql/README.md`, verify and document |
| **M-012** | Add `Client::ping()` method | M | `mssql/src/client/mod.rs` | Lightweight round-trip to verify connection liveness |
| **M-013** | Add `Client::server_version()` method | M | `mssql/src/client/mod.rs` | Return parsed SQL Server version info from login packet |
| **M-014** | Add `Client::database_list()` method | M | `mssql/src/client/mod.rs` | Return list of databases on server. Uses `sp_databases` or sys.dm |
| **M-015** | Add `Client::table_list()` method | M | `mssql/src/client/mod.rs` | Return list of tables in current database. Uses INFORMATION_SCHEMA |
| **M-016** | Add `Row::into_named()` method | M | `mssql/src/row.rs` | Convert `Row` to `HashMap<String, ColumnData<'static>>` |
| **L-006** | Add **query builder** module | L | `mssql/src/query.rs` | Type-safe SQL query construction. Similar to `sqlx::QueryBuilder` but TDS-aware |
| **L-007** | Add **connection pooling** integration | L | `mssql/src/pool.rs` | Built-in support for bb8/mobc/deadpool. Or create adapter crate |

### 2.3 Performance Optimizations

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-024** | Optimize PLP decoding (from issue #226) | S | `mssql/src/tds/codec/column_data/plp.rs` | Bulk read instead of byte-by-byte. Already done per `mssql/issues.md`, verify and benchmark |
| **M-017** | Add **batch query** support | M | `mssql/src/client/mod.rs` | Send multiple queries in one round-trip. Uses MARS or explicit batching |
| **M-018** | Optimize bulk insert for large datasets | M | `mssql/src/client/bulk.rs` | Chunked sends, parallel processing, memory efficiency |
| **M-019** | Add **pipeline** support for query results | M | `mssql/src/client/mod.rs` | Stream results while next query is executing (MARS) |
| **L-008** | Implement **TDS 8.0 full support** | L | `mssql/src/tds/` | All TDS 8.0 features: Strict encryption, new types, protocol improvements |
| **L-009** | Add **SQL Server 2025** support | L | `mssql/src/`, CI | Test and document compatibility with SQL Server 2025 |

### 2.4 Protocol & Type Support

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-025** | Add `FromSql`/`ToSql` for `Uuid` | S | `mssql/src/column_data.rs` | Already exists per `mssql/README.md`, verify and test |
| **S-026** | Add `FromSql`/`ToSql` for `chrono` types | S | `mssql/src/column_data.rs` | chrono feature was removed per `mssql/README.md`, replaced with `jiff` |
| **M-020** | Add `FromSql`/`ToSql` for `json` types | M | `mssql/src/column_data.rs` | Support for SQL Server 2016+ JSON functions |
| **M-021** | Add `FromSql`/`ToSql` for `hierarchyid` | M | `mssql/src/column_data.rs` | SQL Server hierarchyid type support |
| **M-022** | Add `FromSql`/`ToSql` for `geometry`/`geography` | M | `mssql/src/column_data.rs` | SQL Server spatial types. Needs CLR UDT support |
| **M-023** | Add `FromSql`/`ToSql` for `sql_variant` | M | `mssql/src/column_data.rs` | Already done per `mssql/tasks.md`, verify |
| **L-010** | Add **full CLR UDT** support | L | `mssql/src/tds/` | Generic support for all CLR user-defined types |
| **L-011** | Add **Service Broker** support | L | `mssql/src/` | Message queuing for SQL Server. See issue #289 (rejected but revisitable) |

---

## 🧪 3. Testing

### 3.1 Unit Tests

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-027** | Add unit tests for all public API functions | S | `mssql/tests/unit/*.rs` | Every `pub fn` has at least one test. Exclude internal modules |
| **S-028** | Add property-based tests with `proptest` | S | `mssql/Cargo.toml`, `mssql/tests/proptest.rs` | Fuzz parsing, encoding, decoding for edge cases |
| **S-029** | Add tests for all error paths | S | `mssql/tests/*.rs` | Every `Err` variant is tested. Use `assert!(matches!(result, Err(Error::Type(_))))` |
| **M-024** | Add **test coverage** for all feature flags | M | `mssql/tests/`, CI | Each feature flag combination has tests. Use `cargo tarpaulin --features=...` |
| **M-025** | Add **MSRV tests** in CI | M | `.github/workflows/test.yml` | Test on MSRV (current stable - 2). See `spec/rust-msrv-n-minus-2/index.md` |
| **M-026** | Add **cross-platform tests** | M | `.github/workflows/test.yml` | Test on Linux, Windows, macOS for all feature flag combos |

### 3.2 Integration Tests

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-030** | Add **Docker test server** script | S | `mssql/docker/test-server.sh` | Already exists per `mssql/tasks.md`, verify and document |
| **S-031** | Add integration tests for all examples | S | `mssql/examples/*.rs`, `mssql/tests/integration.rs` | Each example runs successfully against test server |
| **M-027** | Add **Azure SQL Edge** CI tests | M | `.github/workflows/test.yml` | Test against Azure SQL Edge container. Already partially done |
| **M-028** | Add **SQL Server 2022** CI tests | M | `.github/workflows/test.yml` | Already done per `mssql/README.md`, verify matrix |
| **M-029** | Add **SQL Server 2019** CI tests | M | `.github/workflows/test.yml` | Already done per `mssql/README.md` |
| **M-030** | Add **SQL Server 2017** CI tests | M | `.github/workflows/test.yml` | Already done per `mssql/README.md` |
| **L-012** | Add **fuzz testing** with `cargo fuzz` | L | `mssql/fuzz/Cargo.toml`, `mssql/fuzz/fuzz_targets/*.rs` | Fuzz TDS packet parsing, encoding, decoding. Integrate with CI |

### 3.3 Test Infrastructure

| # | Task | Size | Touchpoints | Success Points |
|---|------|------|-------------|----------------|
| **S-032** | Add **test helpers** module | S | `mssql/tests/helpers/mod.rs` | Shared test utilities: test server connection, fixture data |
| **S-033** | Add **test data generators** | S | `mssql/tests/helpers/data.rs` | Generate random but valid SQL data for testing |
| **M-031** | Add **test coverage reporting** | M | `.github/workflows/coverage.yml` | Already exists per `mssql/CHANGELOG.md`, verify and enhance |
| **M-032** | Add **benchmark tests** | M | `mssql/benches/*.rs` | Already partially done per `mssql/CHANGELOG.md`, expand coverage |
| **M-033** | Add **performance regression tests** | M | `.github/workflows/bench.yml` | Run benchmarks on PRs, fail if performance degrades beyond threshold |
| **L-013** | Add **test matrix** for all SQL Server versions | L | `.github/workflows/test.yml` | Test against SQL Server 2008→2025, Azure SQL, Azure SQL Edge |

---

## 🔍 4. Auditing

### 4.1 Security Auditing

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-034** | Run `cargo audit` and fix all vulnerabilities | S | `mssql/Cargo.toml`, `mssql/Cargo.lock` | Zero vulnerabilities in `cargo audit --json` output |
| **S-035** | Run `cargo deny` and fix all issues | S | `mssql/deny.toml`, `mssql/Cargo.toml` | Zero issues in `cargo deny check` output. Already partially set up |
| **S-036** | Run `gitleaks` and fix all leaks | S | Repository root | Zero secrets in `gitleaks detect --source . --verbose` output |
| **S-037** | Run `trivy` on Docker images | S | `mssql/docker/Dockerfile*` | Zero vulnerabilities in Docker images. Add to CI |
| **S-038** | Add **security policy** file | S | `.github/SECURITY.md` | Document security reporting process, vulnerability disclosure policy |
| **M-034** | Perform **manual security review** of credential handling | M | `mssql/src/**/*.rs` | Audit all paths that touch passwords, tokens, certificates |
| **M-035** | Perform **manual security review** of network code | M | `mssql/src/tds/`, `mssql/src/client/` | Audit all packet parsing, encoding, TLS handling |
| **M-036** | Add **fuzz testing** for security | M | `mssql/fuzz/` | Fuzz TDS packet parsing for crashes, memory corruption |
| **L-014** | Perform **third-party security audit** | L | External | Engage security firm for professional audit. Publish report |

### 4.2 Code Quality Auditing

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-039** | Run `cargo clippy --all-targets --all-features -- -D warnings` | S | `mssql/src/**/*.rs` | Zero warnings. Check `mssql/CONTRIBUTING.md` |
| **S-040** | Run `cargo fmt --all -- --check` | S | Repository root | All files formatted. Check `mssql/CONTRIBUTING.md` |
| **S-041** | Run `cargo doc --document-private-items --no-deps` | S | `mssql/src/**/*.rs` | Documentation builds without warnings |
| **S-042** | Add **doc tests** for all public API | S | `mssql/src/**/*.rs` | Every `pub fn` has `#[cfg(doc)]` test or `/// # Examples` |
| **M-037** | Run **sonarcloud** analysis | M | `.github/workflows/sonarcloud.yml` | Add SonarCloud analysis to CI. Fix all issues |
| **M-038** | Run **codecov** coverage reporting | M | `.github/workflows/coverage.yml` | Add Codecov integration. Maintain coverage above threshold |
| **M-039** | Add **dependency graph** analysis | M | `.github/dependabot.yml` | Already exists, verify and expand. Add Dependabot for npm (site) |
| **L-015** | Perform **code architecture review** | L | Repository root | Document architecture decisions, identify tech debt, create roadmap |

### 4.3 Compliance Auditing

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-043** | Verify **license compliance** | S | `mssql/LICENSE-MIT.txt`, `mssql/LICENSE-APACHE.txt` | All dependencies have compatible licenses (MIT/Apache-2.0) |
| **S-044** | Verify **attribution compliance** | S | `mssql/README.md`, `mssql/src/lib.rs` | Tiberius attribution is present and accurate per `AGENTS.md` |
| **S-045** | Add **DCO** (Developer Certificate of Origin) | S | `.github/DCO.md`, `.github/workflows/dco.yml` | Add DCO check to PRs. Require `Signed-off-by:` |
| **M-040** | Verify **MSRV compliance** | M | `mssql/Cargo.toml`, `spec/rust-msrv-n-minus-2/index.md` | Code compiles on MSRV (current stable - 2) |
| **M-041** | Verify **semver compliance** | M | `mssql/Cargo.toml`, `mssql/CHANGELOG.md` | All changes follow semver. Breaking changes documented |
| **M-042** | Add **changelog enforcement** | M | `.github/workflows/changelog.yml` | CI checks that CHANGELOG is updated for user-facing changes |

---

## 📊 5. Maintenance & Infrastructure

### 5.1 CI/CD Improvements

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-046** | Add **cache** for cargo builds in CI | S | `.github/workflows/test.yml` | Use `actions/cache` for cargo registry, target directories |
| **S-047** | Add **parallel jobs** for faster CI | S | `.github/workflows/test.yml` | Split test matrix into parallel jobs for different feature flag combos |
| **S-048** | Add **cancel previous runs** for PRs | S | `.github/workflows/test.yml` | Use `concurrency: cancel-in-progress` for PR jobs |
| **M-043** | Add **self-hosted runners** | M | `.github/workflows/`, org settings | Set up self-hosted runners for faster CI on ARM/macOS/Windows |
| **M-044** | Add **nightly builds** | M | `.github/workflows/nightly.yml` | Nightly builds with all features, run tests, upload artifacts |
| **M-045** | Add **release automation** | M | `.github/workflows/publish.yml` | Already exists per `mssql/AI_STATEMENT.md`, verify and enhance. Add changelog generation, GitHub release |
| **L-016** | Migrate to **custom GitHub Actions** | L | `.github/actions/` | Create reusable actions for: setup Rust, setup SQL Server, run tests |

### 5.2 Dependency Management

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-049** | Update **all dependencies** to latest versions | S | `mssql/Cargo.toml`, `mssql/Cargo.lock` | All dependencies at latest compatible versions. Verify with `cargo outdated` |
| **S-050** | Add **Dependabot** for cargo dependencies | S | `.github/dependabot.yml` | Already exists, verify and expand. Add for npm (site) |
| **M-046** | Add **dependency review** workflow | M | `.github/workflows/dependabot-review.yml` | Review Dependabot PRs for security, compatibility, breaking changes |
| **M-047** | Add **dependency audit** to CI | M | `.github/workflows/audit.yml` | Run `cargo audit`, `cargo deny`, `npm audit` on every PR |
| **L-017** | Migrate to **workspace dependencies** | L | `mssql/Cargo.toml` | Use Cargo workspace for shared dependencies across crates |

### 5.3 Project Infrastructure

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-051** | Add **CODEOWNERS** file | S | `.github/CODEOWNERS` | Define code ownership for different parts of the repo |
| **S-052** | Add **ISSUE_TEMPLATE** for bugs | S | `.github/ISSUE_TEMPLATE/bug_report.md` | Template for bug reports with required info (version, OS, SQL Server, reproduction) |
| **S-053** | Add **ISSUE_TEMPLATE** for features | S | `.github/ISSUE_TEMPLATE/feature_request.md` | Template for feature requests with design considerations |
| **S-054** | Add **PULL_REQUEST_TEMPLATE** | S | `.github/PULL_REQUEST_TEMPLATE.md` | Template for PRs with checklist (tests, docs, clippy, fmt) |
| **M-048** | Add **renovate** for dependency updates | M | `.github/renovate.json` | Configure Renovate for automated dependency updates |
| **M-049** | Add **snyk** for security scanning | M | `.github/workflows/snyk.yml` | Add Snyk for container and dependency scanning |
| **L-018** | Migrate **monorepo** to proper workspace | L | Repository root | Use Cargo workspace for both `mssql` crate and site. Or keep separate as documented in `AGENTS.md` |

---

## 📈 6. Analytics & Telemetry

| # | Task | Size | Touchpoints | Success Criteria |
|---|------|------|-------------|------------------|
| **S-055** | Add **Google Analytics** to site | S | `mssql-rust.github.io/src/routes/+layout.svelte` | Track page views, user engagement. Or use privacy-respecting alternative |
| **S-056** | Add **plausible.io** to site | S | `mssql-rust.github.io/src/routes/+layout.svelte` | Privacy-friendly analytics for site |
| **M-050** | Add **download metrics** to README | M | `mssql/README.md` | crates.io download badges with shields.io. Add to site |
| **M-051** | Add **GitHub Insights** to README | M | `mssql/README.md` | GitHub stars, forks, contributors badges. Add to site |
| **M-052** | Add **usage statistics** collection | M | `mssql/src/`, CI | Opt-in telemetry for crate usage (version, features, OS, SQL Server). Anonymous, respect privacy |

---

## 🎯 Priority Recommendations

### Immediate (Next 2 Weeks) - Quick Wins
1. **S-001** - Add "Why this fork?" to site
2. **S-002** - Add interactive "Try it" code block
3. **S-009** - Add Discord/Matrix community links
4. **S-013** - Fix all clippy warnings
5. **S-014** - Fix all cargo fmt issues
6. **S-034** - Run cargo audit and fix vulnerabilities
7. **S-035** - Run cargo deny and fix issues
8. **S-036** - Run gitleaks and fix leaks
9. **S-049** - Update all dependencies
10. **S-051** - Add CODEOWNERS file

### Short Term (Next Month) - High Impact
1. **M-001** - Add feature-compatibility matrix to site
2. **M-002** - Add Quickstart tutorial page
3. **M-007** - Replace String with Zeroizing<String> for credentials
4. **M-012** - Add Client::ping() method
5. **M-024** - Add test coverage for all feature flags
6. **M-027** - Add Azure SQL Edge CI tests
7. **M-034** - Manual security review of credential handling
8. **M-037** - Run sonarcloud analysis
9. **M-043** - Add cache for cargo builds in CI
10. **M-046** - Add dependency review workflow

### Medium Term (Next Quarter) - Strategic
1. **L-001** - Blog post series: "Migrating from Tiberius to mssql"
2. **L-004** - Refactor Client to use builder pattern
3. **L-008** - Implement TDS 8.0 full support
4. **L-012** - Add fuzz testing with cargo fuzz
5. **L-014** - Third-party security audit
6. **M-005** - Create GitHub Discussions templates
7. **M-017** - Add batch query support
8. **M-018** - Optimize bulk insert for large datasets
9. **M-020** - Add FromSql/ToSql for json types
10. **M-031** - Add test helpers module

---

## 📝 How to Use This Catalog

### For AI Agents:
1. **Pick a task** from the appropriate category and priority level
2. **Read the referenced files** (AGENTS.md, CONTRIBUTING.md, spec/) to understand constraints
3. **Check existing work** in tasks.md, issues.md to avoid duplication
4. **Follow the verification discipline** from AGENTS.md: state what you verified and how
5. **Document your work** with proper commit messages and CHANGELOG updates
6. **Add Co-Authored-By trailer** as per AI_STATEMENT.md

### For Human Maintainers:
1. **Assign tasks** to AI agents based on priority and complexity
2. **Review AI work** against the verification criteria in each task
3. **Use the task IDs** (S-001, M-001, etc.) for tracking and discussion
4. **Update this catalog** as tasks are completed or new ones identified
5. **Solicit community feedback** on larger tasks (L-001+)

---

## 🔗 References

- Project Structure: `AGENTS.md`, `mssql/README.md`
- Verification: `mssql/CONTRIBUTING.md`, `AGENTS.md#verification-discipline`
- AI Guidelines: `AI_STATEMENT.md`, `mssql/spec/ai-release-authorization/index.md`
- Work Tracking: `mssql/tasks.md`, `mssql/issues.md`
- Specifications: `spec/index.md`, `mssql/spec/`
- CI/CD: `.github/workflows/`
- Site: `mssql-rust.github.io/`
