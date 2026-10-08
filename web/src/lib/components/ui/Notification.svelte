<script lang="ts">
	import type { Snippet } from 'svelte';

	let { label, floating = false, anchored = false, leaving = false, icon, actions, children }: {
		label: string;
		floating?: boolean;
		anchored?: boolean;
		leaving?: boolean;
		icon: Snippet;
		actions?: Snippet;
		children: Snippet;
	} = $props();
</script>

<div class={floating ? `pointer-events-none ${anchored ? 'absolute top-4' : 'fixed top-12'} left-1/2 z-50 w-80 max-w-[calc(100%-2rem)] -translate-x-1/2` : 'min-w-0 w-full'}>
	<aside aria-label={label} class="notice pointer-events-auto flex items-start gap-2.5 rounded-lg border border-subtle bg-raised px-3 py-2.5 text-text" class:floating class:leaving>
		<div class="mt-0.5 shrink-0">{@render icon()}</div>
		<div class="min-w-0 flex-1">{@render children()}</div>
		{#if actions}<div class="-mt-0.5 shrink-0">{@render actions()}</div>{/if}
	</aside>
</div>

<style>
	.floating { box-shadow: 0 4px 16px #0001; animation: notice-in 180ms var(--ease-out) both; }
	.leaving { animation: notice-out 180ms var(--ease-standard) both; }
	@keyframes notice-in {
		from { opacity: 0; transform: translateY(-8px); }
		to { opacity: 1; transform: translateY(0); }
	}
	@keyframes notice-out {
		from { opacity: 1; transform: translateY(0); }
		to { opacity: 0; transform: translateY(-8px); }
	}
</style>
