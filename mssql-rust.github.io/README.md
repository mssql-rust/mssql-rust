# mssql-rust.github.io

The landing page for **[mssql](https://github.com/mssql-rust/mssql-rust)** — a native,
asynchronous Rust client for Microsoft SQL Server (TDS), forked from
[Tiberius](https://github.com/prisma/tiberius) to prioritize ongoing maintenance and
security updates.

Published at <https://mssql-rust.github.io/>.

## Toolchain

- **[SvelteKit](https://svelte.dev/docs/kit)** with **[adapter-static](https://svelte.dev/docs/kit/adapter-static)** — every route is prerendered to plain HTML, which is all GitHub Pages serves.
- **[Lily Design System](https://github.com/LilyDesignSystem)** — the Svelte headless components and helpers supply the semantics and accessibility; a Lily theme stylesheet supplies the design tokens and component styling.

There is no runtime JavaScript requirement for reading: the prerendered HTML is
complete. Client-side JavaScript adds the theme picker, the text-size picker, and
the clipboard-copy button on the install command.

Requires Node.js 22+ (see `.tool-versions`; `vite` needs `node:util`'s
`styleText`, added in Node 21.7/22).

## Working locally

```sh
npm install
npm run dev        # dev server with live reload
npm run build      # prerender the whole site into build/
npm run preview    # serve build/ exactly as GitHub Pages will
```

## Keeping Lily in sync

The Lily components, helpers, and theme stylesheets are vendored, so the site
builds anywhere without the sibling checkout present.

```sh
npm run sync:lily  # copy the Lily components, helpers, and themes
```

`npm run sync:lily` reads `~/git/lilydesignsystem/lily-design-system` by
default; set `LILY=/path/to/lily-design-system` to override. It overwrites
what it manages, so re-running it is the way to pick up upstream changes.
Commit the result.

The vendored files and their upstream commit are recorded in
[`src/lib/lily/VENDOR.md`](src/lib/lily/VENDOR.md). Do not edit them there —
change them upstream and re-sync.

Not every Lily component the page uses is vendored: `HeroHeadline`,
`CodeBlock`, `Code`, and `Grid` are each a plain `<div>` with an `aria-label`
and a class name, so this site applies the same Lily class
(`.hero-headline`, `.code-block`, `.code`) directly to the right semantic
HTML element instead (a classed `<h1>`, a `<pre><code>`) — see
`bin/sync-lily.mjs` and `src/routes/+page.svelte`.

## Themes and accessibility

The header carries two Lily helpers:

- **Text size** — small, medium, large, x-large, persisted in `localStorage`.
- **Theme** — Light, Dark, Nord, and Dracula. The choice is persisted, and the
  first visit follows the operating system's light/dark preference.

Themes are plain stylesheets in `static/themes/`; the picker swaps the
managed `<link>` in `src/app.html`.

## Deployment

`.github/workflows/deploy.yml` builds and deploys on every push to `main`. For
the first deployment, set **Settings → Pages → Build and deployment → Source
→ GitHub Actions** in the repository.

The setup follows the SvelteKit guidance in
[adapter-static → GitHub Pages](https://svelte.dev/docs/kit/adapter-static#GitHub-Pages):

- `fallback: '404.html'` in [`svelte.config.js`](svelte.config.js), so a
  wrong URL gets this site's own error page instead of GitHub's default 404;
- an empty [`static/.nojekyll`](static/.nojekyll), so GitHub does not run
  Jekyll over the build output;
- `paths.base` left empty. That guidance's `BASE_PATH` step is only for
  project pages served from `https://<owner>.github.io/<repo>/`. This
  repository is named after the organization, so the site is served from the
  root and every link can stay absolute.
