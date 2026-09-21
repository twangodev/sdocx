<script lang="ts">
	import { Sun, Moon } from '@lucide/svelte';
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
			<Sun
				size={16}
				strokeWidth={1.7}
				class="animate-[theme-icon-in_var(--motion-standard)_var(--ease-out)_both]"
				aria-hidden="true"
			/>
		{:else}
			<Moon
				size={16}
				strokeWidth={1.7}
				class="animate-[theme-icon-in_var(--motion-standard)_var(--ease-out)_both]"
				aria-hidden="true"
			/>
		{/if}
	{/key}
</button>
