<script lang="ts">
	import './layout.css';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';

	let { children } = $props();
</script>

<div class="shell">
	<header>
		<a class="wordmark" href="/" aria-label="sdocx home"><span>s</span>docx</a>
		<nav aria-label="Primary navigation">
			<a href="https://github.com/twangodev/sdocx">github</a>
			<ThemeToggle />
		</nav>
	</header>
	<main>{@render children()}</main>
	<footer>
		<span>Files are processed locally in your browser.</span>
	</footer>
</div>

<style>
	.shell {
		display: flex;
		width: min(100%, 1320px);
		min-height: 100svh;
		margin: 0 auto;
		flex-direction: column;
		padding: 1.25rem clamp(1rem, 3vw, 2.5rem);
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
		gap: 1rem;
	}

	nav a {
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.68rem;
		text-decoration: none;
		text-transform: lowercase;
	}

	nav a:hover {
		color: #167bff;
	}

	main {
		display: flex;
		flex: 1;
		align-items: stretch;
		padding: clamp(2rem, 5vw, 4rem) 0;
	}

	footer {
		border-top: 1px solid var(--site-border);
		padding-top: 1rem;
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.62rem;
	}
</style>
