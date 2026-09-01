<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { Snippet } from 'svelte';

	let { text, children }: { text: string; children: Snippet<[Record<string, unknown>]> } =
		$props();
</script>

<Tooltip.Provider delayDuration={350}>
	<Tooltip.Root>
		<Tooltip.Trigger>
			{#snippet child({ props })}
				{@render children(props)}
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Portal>
			<Tooltip.Content
				sideOffset={6}
				class="motion-tooltip border-subtle bg-bg z-50 max-w-64 rounded border px-2.5 py-1.5 text-[11px] tracking-wide text-muted lowercase shadow-xl"
			>
				{text}
				<Tooltip.Arrow class="fill-bg" />
			</Tooltip.Content>
		</Tooltip.Portal>
	</Tooltip.Root>
</Tooltip.Provider>
