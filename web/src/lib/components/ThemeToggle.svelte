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

<button
	class="grid size-[1.85rem] cursor-pointer place-items-center rounded border-0 bg-transparent text-muted transition-[background-color,color,transform] duration-[var(--motion-fast)] ease-[var(--ease-standard)] hover:bg-surface hover:text-text"
	type="button"
	onclick={toggle}
	aria-label={`Use ${theme === 'dark' ? 'light' : 'dark'} theme`}
>
	{#key theme}
		{#if theme === 'dark'}
			<svg class="size-4 animate-[theme-icon-in_var(--motion-standard)_var(--ease-out)_both] fill-none stroke-current [stroke-linecap:round] [stroke-linejoin:round] [stroke-width:1.7]" aria-hidden="true" viewBox="0 0 24 24"><circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M4.93 4.93l1.42 1.42m11.3 11.3 1.42 1.42M2 12h2m16 0h2M4.93 19.07l1.42-1.42m11.3-11.3 1.42-1.42"/></svg>
		{:else}
			<svg class="size-4 animate-[theme-icon-in_var(--motion-standard)_var(--ease-out)_both] fill-none stroke-current [stroke-linecap:round] [stroke-linejoin:round] [stroke-width:1.7]" aria-hidden="true" viewBox="0 0 24 24"><path d="M20.4 15.5A9 9 0 0 1 8.5 3.6 9 9 0 1 0 20.4 15.5Z"/></svg>
		{/if}
	{/key}
</button>
