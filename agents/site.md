# Editing `mssql-rust.github.io/`

A SvelteKit static site (adapter-static), deployed to GitHub Pages at
https://mssql-rust.github.io/ by
[`.github/workflows/deploy.yml`](../mssql-rust.github.io/.github/workflows/deploy.yml)
on every push to `main`. It is not published anywhere else, and has no
relationship to crates.io publishing — see [`release.md`](release.md).

## Toolchain

Node version is pinned in `.tool-versions` (currently `22.23.2`). If the
ambient shell's `node -v` looks older, use `mise exec -- npm ...` rather
than assuming the pin is wrong. `npm ci` (not `npm install`) in CI and when
you want exactly the locked versions; use plain `npm install` only when
deliberately changing a dependency.

## Lily Design System vendoring

`src/lib/lily/` is vendored, not a dependency — copied in by
[`bin/sync-lily.mjs`](../mssql-rust.github.io/bin/sync-lily.mjs) from a
sibling checkout (`$LILY` or `~/git/lilydesignsystem/lily-design-system`),
because Lily's Svelte headless components/helpers aren't published to npm
and the local checkout runs ahead of what is. The source commit synced is
recorded in
[`src/lib/lily/VENDOR.md`](../mssql-rust.github.io/src/lib/lily/VENDOR.md).

- Don't hand-edit files under `src/lib/lily/` — re-run `npm run sync:lily`
  against an updated Lily checkout instead, so `VENDOR.md` stays truthful
  about what's actually vendored.
- Before re-syncing, check how far behind the vendored commit is (`git log
  <vendored-sha>..HEAD` in the Lily checkout) and skim what changed — a sync
  pulls in whatever Lily has done since, unreviewed, by design of the
  script.

## Dependency audit

`npm audit` findings against a devDependency's own transitive pin (not
something this project's `package.json` declares directly) may not have a
non-breaking upstream fix yet — check the resolved version
(`npm ls <package>`) and whether the *latest* version of the direct
dependency still pulls the vulnerable range before reaching for
`npm audit fix --force`, which often suggests a large downgrade instead of a
real fix. An `overrides` entry pinning just the transitive package to a
fixed, semver-adjacent version (see the existing `cookie` override in
`package.json`, needed because `@sveltejs/kit@2.70.x` itself still declares
`"cookie": "^0.6.0"`) is usually the safer move — verify the build and, if
the site actually uses the overridden package's behavior, the relevant
functionality still works after overriding.

## Keeping content in sync with the crate

The landing page (`src/routes/+page.svelte`) makes factual claims about the
`mssql` crate (feature list, version-support table, TLS/auth options). When
a crate change makes one of these claims stale — a new feature flag worth a
mention, a changed SQL Server version support matrix — update the page in
the same change, not as a follow-up. Cross-check against `../mssql/Cargo.toml`
and `../mssql/README.md` directly; don't trust your memory of what the crate
does.

`static/llms.json` and `static/llms.txt` are **not** copies of
`../mssql/llms.json`/`llms.txt` — they're site-appropriate versions whose
links resolve from this site's own domain. See
[`../spec/llms-json-and-llms-txt/index.md`](../spec/llms-json-and-llms-txt/index.md)
before touching either.

## Verifying a change

`npm run build` (needs `node_modules` — run `npm ci` first if absent) is the
real check: it's a static adapter, so a successful build is close to a
successful deploy. `npm run check` currently only runs `svelte-kit sync`
(regenerates generated types) — it does **not** run `svelte-check`, so it
won't catch a type error in a `.svelte` file. Don't rely on it as a
type-check; if you need one, run `npx svelte-check` directly (it isn't a
devDependency yet).
