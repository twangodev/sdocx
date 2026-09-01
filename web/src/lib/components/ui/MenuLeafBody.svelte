<script lang="ts">
	import { Check } from '@lucide/svelte';
	import type { MenuAction } from '$lib/menu';

	let { entry }: { entry: MenuAction<unknown> } = $props();
</script>

<span class="flex w-3 items-center justify-center">
	{#if entry.checked}<Check size={10} />{/if}
</span>
<span class="flex-1">{entry.label}</span>
{#if entry.shortcut}
	<kbd class="ml-5 font-mono text-[10px] text-muted">{entry.shortcut}</kbd>
{/if}
