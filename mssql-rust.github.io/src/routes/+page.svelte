<script>
	import ArticleLayout from '$lib/lily/components/ArticleLayout.svelte';
	import Hero from '$lib/lily/components/Hero.svelte';
	import ButtonGroup from '$lib/lily/components/ButtonGroup.svelte';
	import ActionLink from '$lib/lily/components/ActionLink.svelte';
	import Card from '$lib/lily/components/Card.svelte';
	import FeatureCard from '$lib/lily/components/FeatureCard.svelte';
	import Badge from '$lib/lily/components/Badge.svelte';
	import InformationCallout from '$lib/lily/components/InformationCallout.svelte';
	import ClipboardCopyButton from '$lib/lily/components/ClipboardCopyButton.svelte';
	import {
		CRATES_IO,
		DOCS_RS,
		REPOSITORY,
		SITE_NAME,
		SITE_TAGLINE,
		SITE_URL,
		TIBERIUS
	} from '$lib/site.js';

	const installCommand = 'cargo add mssql';
	let copied = $state(false);

	const features = [
		{
			heading: 'Three TLS backends',
			description:
				'rustls, native-tls, or vendored OpenSSL — pick the one that fits your platform and deployment, including TDS 8.0 "Strict" encryption.'
		},
		{
			heading: 'Flexible authentication',
			description:
				'SQL Server logins, Windows/NTLM (including a pure-Rust backend for Unix), Kerberos/GSSAPI, and Azure AD tokens.'
		},
		{
			heading: 'Runtime-independent',
			description:
				'Built on futures-rs traits, not a specific executor — works with Tokio, async-std, or smol.'
		},
		{
			heading: 'Bulk insert, done well',
			description:
				'SqlBulkCopyOptions, column selection, ORDER hints, and CA-certificate bundles for locked-down environments.'
		},
		{
			heading: 'A clear MSRV policy',
			description:
				'Minimum supported Rust version tracks current stable minus two minor releases, enforced in CI.'
		},
		{
			heading: 'Security-focused maintenance',
			description:
				'Regular dependency upgrades, cargo-audit review, and fixes for real protocol bugs found via live-server testing.'
		}
	];
</script>

<svelte:head>
	<title>{SITE_NAME} — {SITE_TAGLINE}</title>
	<meta
		name="description"
		content="mssql is a native, async Rust client for Microsoft SQL Server (TDS), forked from Tiberius to prioritize ongoing maintenance and security updates."
	/>
	<link rel="canonical" href={`${SITE_URL}/`} />
	<meta property="og:title" content={`${SITE_NAME} — ${SITE_TAGLINE}`} />
	<meta
		property="og:description"
		content="A native, async Rust client for Microsoft SQL Server (TDS), forked from Tiberius to prioritize ongoing maintenance and security updates."
	/>
	<meta property="og:type" content="website" />
	<meta property="og:url" content={`${SITE_URL}/`} />
</svelte:head>

<Hero label="Introduction">
	<h1 class="hero-headline">{SITE_NAME}</h1>
	<p class="hero-tagline">
		A native, asynchronous Rust client for Microsoft SQL Server, speaking the TDS protocol
		directly — no ODBC, no unsafe FFI.
	</p>
	<ButtonGroup label="Primary actions" class="hero-actions">
		<ActionLink class="button" href={DOCS_RS}>Read the docs</ActionLink>
		<ActionLink class="button button-secondary" href={REPOSITORY}>View on GitHub</ActionLink>
	</ButtonGroup>
</Hero>

<InformationCallout label="Note" class="fork-callout">
	<p>
		<strong>{SITE_NAME}</strong> is a friendly fork of
		<a href={TIBERIUS}>Tiberius</a>, the TDS client originally created by Prisma and its
		contributors. Many thanks to the Tiberius team and community for the work this fork builds
		on — full commit history, same MIT/Apache-2.0 licenses, same goals. This fork exists to
		prioritize ongoing maintenance and security updates: current SQL Server and TDS protocol
		versions, small focused changes over large rewrites.
	</p>
</InformationCallout>

<ArticleLayout label="Getting started" class="install-section">
	<h2>Install</h2>
	<div class="install-row">
		<pre class="code-block install-command"><code class="code">{installCommand}</code></pre>
		<ClipboardCopyButton
			class="button"
			text={installCommand}
			label="Copy install command"
			onsuccess={() => (copied = true)}
		>
			{copied ? 'Copied' : 'Copy'}
		</ClipboardCopyButton>
	</div>
	<p>
		Or add it to <code class="code">Cargo.toml</code> directly, then see the
		<a href={DOCS_RS}>API documentation</a> for connecting, querying, and bulk-inserting rows.
	</p>
</ArticleLayout>

<section class="feature-section" aria-labelledby="features-heading">
	<h2 id="features-heading">Why mssql</h2>
	<div class="feature-grid">
		{#each features as feature (feature.heading)}
			<FeatureCard heading={feature.heading} description={feature.description} />
		{/each}
	</div>
</section>

<ArticleLayout label="Supported SQL Server versions" class="versions-section">
	<h2>Supported SQL Server versions</h2>
	<p>Tested against these versions on every commit, in CI, over a real TLS connection:</p>
	<div class="version-badges">
		<Badge type="success">2022</Badge>
		<Badge type="success">2019</Badge>
		<Badge type="success">2017</Badge>
		<Badge>2016 and earlier — should work</Badge>
	</div>
</ArticleLayout>

<Card heading="Read the source" href={REPOSITORY} class="source-card">
	<p>
		The crate's full history — including every commit from Tiberius before the fork — lives at
		<a href={REPOSITORY}>{REPOSITORY.replace('https://', '')}</a>, mirrored on GitLab and
		Codeberg. Published to <a href={CRATES_IO}>crates.io</a> as <code class="code">mssql</code>.
	</p>
</Card>
