<script>
	import { page } from '$app/state';
	import SkipLink from '$lib/lily/components/SkipLink.svelte';
	import Header from '$lib/lily/components/Header.svelte';
	import Footer from '$lib/lily/components/Footer.svelte';
	import ThemePicker from '$lib/lily/helpers/ThemePicker.svelte';
	import TextSizePicker from '$lib/lily/helpers/TextSizePicker.svelte';
	import SharePicker from '$lib/lily/helpers/SharePicker.svelte';
	import { SHARE_TARGETS, promptMastodonShare } from '$lib/share.js';
	import {
		CODEBERG,
		DOCS_RS,
		CRATES_IO,
		GITLAB,
		REPOSITORY,
		SITE_NAME,
		THEMES,
		THEME_LABELS,
		TIBERIUS
	} from '$lib/site.js';
	import '../styles/site.css';

	let { children } = $props();

	const shareTitle = $derived(page.data.title ?? SITE_NAME);

	// Mastodon has no single share-intent URL (see src/lib/share.js), so its
	// link is intercepted here — SharePicker spreads `onclick` onto its root,
	// and the click bubbles up from the `<a data-target-id="mastodon">` it
	// renders for the target below.
	function onShareClick(event) {
		const link = event.target.closest?.('a[data-target-id="mastodon"]');
		if (!link) return;
		event.preventDefault();
		promptMastodonShare(page.url.href, shareTitle);
	}
</script>

<SkipLink href="#main" label="Skip to main content" />

<Header label="Site header" class="site-header">
	<div class="site-header-inner">
		<a class="site-brand" href="/">
			<img src="/favicon.svg" alt="" aria-hidden="true" width="28" height="28" />
			<span>{SITE_NAME}</span>
		</a>
		<nav class="site-nav" aria-label="Main">
			<a href={DOCS_RS}>Docs</a>
			<a href={CRATES_IO}>Crates.io</a>
			<a href={REPOSITORY}>GitHub</a>
		</nav>
		<div class="site-tools">
			<TextSizePicker
				label="Text size"
				sizes={['small', 'medium', 'large', 'x-large']}
				storageKey="mssql-rust-text-size"
			/>
			<ThemePicker
				label="Theme"
				themesUrl="/themes/"
				themes={THEMES}
				themeLabels={THEME_LABELS}
				storageKey="mssql-rust-theme"
				detectFromSystem
			/>
			<SharePicker
				label="Share"
				targets={SHARE_TARGETS}
				url={page.url.href}
				title={shareTitle}
				onclick={onShareClick}
			/>
		</div>
	</div>
</Header>

<main id="main" class="site-main">
	{@render children()}
</main>

<Footer label="Site footer" class="site-footer">
	<div class="site-footer-inner">
		<p>
			<strong>{SITE_NAME}</strong> is a community fork of
			<a href={TIBERIUS}>Tiberius</a>, prioritizing ongoing maintenance and security updates for
			SQL Server's TDS protocol. Same MIT/Apache-2.0 licenses. Built with the
			<a href="https://github.com/LilyDesignSystem">Lily Design System</a>.
		</p>
		<div class="site-footer-links">
			<a href={REPOSITORY}>GitHub</a>
			<a href={GITLAB}>GitLab</a>
			<a href={CODEBERG}>Codeberg</a>
			<a href={CRATES_IO}>Crates.io</a>
			<a href={DOCS_RS}>Docs.rs</a>
		</div>
	</div>
</Footer>
