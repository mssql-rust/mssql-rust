import { SITE_NAME, SITE_TAGLINE } from '$lib/site.js';

// `data.title` is the page.data.title convention: every route that wants a
// share-able title (SharePicker in +layout.svelte reads `page.data.title`)
// exports it from its `load`, rather than +layout.svelte reaching into each
// page's own <svelte:head>.
export const load = () => ({
	title: `${SITE_NAME} — ${SITE_TAGLINE}`
});
