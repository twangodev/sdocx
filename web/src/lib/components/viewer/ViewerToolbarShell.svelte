<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		label: string;
		start: Snippet;
		center: Snippet;
		end: Snippet;
	}

	let { label, start, center, end }: Props = $props();
</script>

<header
	class="grid min-h-11 shrink-0 grid-cols-[minmax(10rem,1fr)_auto_minmax(10rem,1fr)] items-center gap-2 border-b border-subtle bg-bg px-2 py-1 max-[820px]:grid-cols-[minmax(0,1fr)_auto]"
	aria-label={label}
>
	<div class="min-w-0">{@render start()}</div>
	<div class="justify-self-center max-[820px]:order-3 max-[820px]:col-span-2">
		{@render center()}
	</div>
	<div class="min-w-0 justify-self-end">{@render end()}</div>
</header>
