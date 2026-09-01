<script lang="ts" generics="A">
	import { DropdownMenu } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import {
		menuCompactContentClass,
		menuCompactItemClass,
		menuContentClass,
		menuItemClass,
		menuSeparatorClass,
		type MenuLeaf
	} from '$lib/menu';
	import MenuLeafBody from './MenuLeafBody.svelte';

	interface Props {
		items: MenuLeaf<A>[];
		onAction: (action: A) => void;
		align?: 'start' | 'center' | 'end';
		size?: 'default' | 'compact';
		children: Snippet<[{ props: Record<string, unknown> }]>;
	}

	let { items, onAction, align = 'start', size = 'default', children }: Props = $props();

	const itemClass = $derived(size === 'compact' ? menuCompactItemClass : menuItemClass);
	const contentClass = $derived(size === 'compact' ? menuCompactContentClass : menuContentClass);

	function select(entry: MenuLeaf<A>) {
		if (entry.kind === 'action') onAction(entry.action);
	}
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger>
		{#snippet child({ props })}
			{@render children({ props })}
		{/snippet}
	</DropdownMenu.Trigger>
	<DropdownMenu.Portal>
		<DropdownMenu.Content {align} sideOffset={4} class={contentClass}>
			{#each items as entry, index (index)}
				{#if entry.kind === 'separator'}
					<DropdownMenu.Separator class={menuSeparatorClass} />
				{:else}
					<DropdownMenu.Item
						disabled={entry.disabled ?? false}
						class="{itemClass} motion-menu-item"
						style={`--motion-index: ${index}`}
						onSelect={() => select(entry)}
					>
						<MenuLeafBody {entry} />
					</DropdownMenu.Item>
				{/if}
			{/each}
		</DropdownMenu.Content>
	</DropdownMenu.Portal>
</DropdownMenu.Root>
