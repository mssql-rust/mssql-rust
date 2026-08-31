// Share-target URL builders for SharePicker
// (src/lib/lily/helpers/SharePicker.svelte, vendored — see VENDOR.md).
//
// Mastodon has no single share-intent URL: it's federated, so there is no
// equivalent of bsky.app/intent/compose. `promptMastodonShare` asks the
// visitor for their home instance on first share and remembers it in
// localStorage; the `mastodon` target's `href` below is only the fallback
// for a middle-click / copy-link-address / no-JS visitor, so it points at
// a working default instance rather than nothing.

const MASTODON_INSTANCE_KEY = 'mssql-rust-mastodon-instance';
const MASTODON_DEFAULT_INSTANCE = 'mastodon.social';

function shareText(title, url) {
	return title ? `${title} ${url}` : url;
}

function normalizeInstance(input) {
	return input.trim().replace(/^https?:\/\//, '').replace(/\/+$/, '');
}

/** The visitor's remembered Mastodon instance, or null if none is stored yet. */
export function mastodonInstance() {
	try {
		return localStorage.getItem(MASTODON_INSTANCE_KEY);
	} catch {
		// ignore quota / privacy errors
		return null;
	}
}

export function mastodonShareUrl(instance, url, title) {
	return `https://${instance}/share?text=${encodeURIComponent(shareText(title, url))}`;
}

/**
 * Handles a click on the SharePicker's Mastodon link: on the first share,
 * prompts for the visitor's home instance and remembers it; afterwards,
 * shares straight there. Call this after `event.preventDefault()` from a
 * click handler on SharePicker's root — see the `onclick` prop wired in
 * +layout.svelte, which spreads onto the root via SharePicker's
 * `restProps` and delegates for `a[data-target-id="mastodon"]`.
 */
export function promptMastodonShare(url, title) {
	let instance = mastodonInstance();
	if (!instance) {
		const input = window.prompt('Share to which Mastodon instance? (e.g. mastodon.social)');
		if (!input) return;
		instance = normalizeInstance(input);
		if (!instance) return;
		try {
			localStorage.setItem(MASTODON_INSTANCE_KEY, instance);
		} catch {
			// ignore quota / privacy errors
		}
	}
	window.open(mastodonShareUrl(instance, url, title), '_blank', 'noopener,noreferrer');
}

/** SharePicker targets, in menu order. */
export const SHARE_TARGETS = [
	{
		id: 'linkedin',
		label: 'LinkedIn',
		href: (url) => `https://www.linkedin.com/sharing/share-offsite/?url=${encodeURIComponent(url)}`
	},
	{
		id: 'mastodon',
		label: 'Mastodon',
		href: (url, title) =>
			mastodonShareUrl(mastodonInstance() ?? MASTODON_DEFAULT_INSTANCE, url, title)
	},
	{
		id: 'bluesky',
		label: 'Bluesky',
		href: (url, title) => `https://bsky.app/intent/compose?text=${encodeURIComponent(shareText(title, url))}`
	},
	{
		id: 'reddit',
		label: 'Reddit',
		href: (url, title) =>
			`https://www.reddit.com/submit?url=${encodeURIComponent(url)}&title=${encodeURIComponent(title)}`
	}
];
