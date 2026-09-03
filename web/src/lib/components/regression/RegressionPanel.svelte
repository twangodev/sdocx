<script lang="ts">
	import { ChevronDown } from '@lucide/svelte';
	import { Collapsible } from 'bits-ui';
	import { cubicOut } from 'svelte/easing';
	import { slide } from 'svelte/transition';
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		meta?: string;
		open?: boolean;
		children: Snippet;
	}

	let { title, meta, open = $bindable(true), children }: Props = $props();
</script>

<Collapsible.Root bind:open class="border-b border-subtle">
	<Collapsible.Trigger
		class="group flex h-10 w-full cursor-pointer items-center justify-between px-3 text-left"
	>
		<span class="text-[11px] tracking-[0.03em] text-text/85 lowercase">{title}</span>
		<span class="flex items-center gap-2">
			{#if meta}<span class="font-mono text-[10px] text-muted">{meta}</span>{/if}
			<ChevronDown
				size={13}
				strokeWidth={1.5}
				class="text-muted transition-transform group-data-[state=open]:rotate-180"
			/>
		</span>
	</Collapsible.Trigger>
	<Collapsible.Content forceMount>
		{#snippet child({ props, open: expanded })}
			{#if expanded}
				<div {...props} transition:slide={{ duration: 180, easing: cubicOut }}>
					<div class="px-3 pb-3">{@render children()}</div>
				</div>
			{/if}
		{/snippet}
	</Collapsible.Content>
</Collapsible.Root>
