<script lang="ts">
	import { page } from '$app/state';
	import './layout.css';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';

	let { children } = $props();
</script>

<div class="shell">
	<header>
		<a class="wordmark" href="/" aria-label="sdocx home"><span>s</span>docx</a>
		<nav aria-label="Primary navigation">
			<a href="/" aria-current={page.url.pathname === '/' ? 'page' : undefined}>convert</a>
			<a
				href="/regressions"
				aria-current={page.url.pathname.startsWith('/regressions') ? 'page' : undefined}
				>regressions</a
			>
			<a href="https://github.com/twangodev/sdocx">github</a>
			<ThemeToggle />
		</nav>
	</header>
	<main>{@render children()}</main>
	<footer>
		<span>Samsung Notes, understood in the open.</span>
		<span class="privacy">Files stay on this device.</span>
	</footer>
</div>

<style>
	.shell {
		display: flex;
		width: min(100%, 1600px);
		min-height: 100svh;
		margin: 0 auto;
		flex-direction: column;
		padding: 1.25rem clamp(1rem, 3vw, 3.5rem);
	}

	header,
	footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
	}

	header {
		min-height: 3rem;
	}

	.wordmark {
		color: var(--site-text);
		font-family: var(--font-mono);
		font-size: 1rem;
		font-weight: 650;
		letter-spacing: -0.04em;
		text-decoration: none;
	}

	.wordmark span {
		color: #167bff;
	}

	nav {
		display: flex;
		align-items: center;
		gap: clamp(0.75rem, 2vw, 1.6rem);
	}

	nav a {
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.68rem;
		text-decoration: none;
		text-transform: lowercase;
	}

	nav a:hover,
	nav a[aria-current='page'] {
		color: #167bff;
	}

	main {
		display: flex;
		flex: 1;
		align-items: stretch;
		padding: clamp(2rem, 6vw, 5rem) 0;
	}

	footer {
		border-top: 1px solid var(--site-border);
		padding-top: 1rem;
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.62rem;
	}

	@media (max-width: 600px) {
		nav a:nth-child(3),
		.privacy {
			display: none;
		}
	}
</style>
