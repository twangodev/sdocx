<script lang="ts">
	import './layout.css';
	import Logo from '$lib/components/Logo.svelte';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';

	let { children } = $props();
</script>

<div class="shell">
	<header>
		<a class="wordmark" href="/" aria-label="sdocx home"><Logo size={15} />sdocx</a>
		<nav aria-label="Primary navigation">
			<a href="https://github.com/twangodev/sdocx" target="_blank" rel="noreferrer">github</a>
			<ThemeToggle />
		</nav>
	</header>
	<main>{@render children()}</main>
</div>

<style>
	.shell {
		display: flex;
		width: 100%;
		min-height: 100svh;
		flex-direction: column;
	}

	header {
		display: flex;
		min-height: 2.5rem;
		align-items: center;
		justify-content: space-between;
		gap: 0.65rem;
		border-bottom: 1px solid var(--site-border);
		padding: 0 0.7rem;
	}

	.wordmark {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		color: var(--site-text);
		font-size: 0.9rem;
		font-weight: 550;
		letter-spacing: 0.01em;
		text-decoration: none;
		transition:
			color var(--motion-fast) var(--ease-standard),
			transform var(--motion-control) var(--ease-out);
	}

	.wordmark:hover {
		color: #167bff;
	}

	nav {
		display: flex;
		align-items: center;
		gap: 0.55rem;
	}

	nav a {
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.68rem;
		text-decoration: none;
		text-transform: lowercase;
		transition: color var(--motion-fast) var(--ease-standard);
	}

	nav a:hover {
		color: #167bff;
	}

	main {
		display: flex;
		min-height: 0;
		flex: 1;
		align-items: stretch;
	}
</style>
