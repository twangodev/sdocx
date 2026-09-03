<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		id: string;
		title: string;
		description: string;
		aside?: Snippet;
		class?: string;
	}

	let { id, title, description, aside, class: className = '' }: Props = $props();
</script>

<div class="mb-4 flex items-start justify-between gap-5 {className}">
	<div>
		<h3 class="m-0 font-[610] tracking-[-0.025em]" {id}>{title}</h3>
		<p class="mt-1 mb-0 text-[0.78rem] text-muted">{description}</p>
	</div>
	{#if aside}{@render aside()}{/if}
</div>
