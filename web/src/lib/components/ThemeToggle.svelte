<script lang="ts">
	let theme = $state<'light' | 'dark'>('dark');

	$effect(() => {
		const current = document.documentElement.dataset.theme;
		theme = current === 'light' ? 'light' : 'dark';
	});

	function toggle(): void {
		theme = theme === 'dark' ? 'light' : 'dark';
		document.documentElement.dataset.theme = theme;
		document.documentElement.style.colorScheme = theme;
		localStorage.setItem('sdocx-theme', theme);
	}
</script>

<button class="theme-toggle" type="button" onclick={toggle} aria-label={`Use ${theme === 'dark' ? 'light' : 'dark'} theme`}>
	{#if theme === 'dark'}
		<svg aria-hidden="true" viewBox="0 0 24 24"><circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M4.93 4.93l1.42 1.42m11.3 11.3 1.42 1.42M2 12h2m16 0h2M4.93 19.07l1.42-1.42m11.3-11.3 1.42-1.42"/></svg>
	{:else}
		<svg aria-hidden="true" viewBox="0 0 24 24"><path d="M20.4 15.5A9 9 0 0 1 8.5 3.6 9 9 0 1 0 20.4 15.5Z"/></svg>
	{/if}
</button>

<style>
	.theme-toggle {
		display: grid;
		width: 2.25rem;
		height: 2.25rem;
		place-items: center;
		border: 0;
		border-radius: 0.25rem;
		background: transparent;
		color: var(--site-muted);
		cursor: pointer;
		transition: background-color 140ms ease, color 140ms ease;
	}

	.theme-toggle:hover {
		background: var(--site-surface);
		color: var(--site-text);
	}

	svg {
		width: 1rem;
		height: 1rem;
		fill: none;
		stroke: currentColor;
		stroke-linecap: round;
		stroke-linejoin: round;
		stroke-width: 1.7;
	}
</style>
