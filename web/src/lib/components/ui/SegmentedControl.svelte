<script lang="ts" generics="T extends string">
	import { ToggleGroup } from 'bits-ui';
	import type { Snippet } from 'svelte';

	interface Props {
		options: readonly T[];
		value: T;
		label: string;
		disabled?: boolean;
		itemLabel?: (option: T) => string;
		itemClass?: string;
		itemStyle?: (option: T) => string;
		itemTitle?: (option: T) => string;
		item?: Snippet<[T, boolean]>;
		class?: string;
	}

	let {
		options,
		value = $bindable(),
		label,
		disabled = false,
		itemLabel = (option) => option,
		itemClass = 'border-subtle text-[11px] text-muted lowercase hover:text-text data-[state=on]:border-control-edge data-[state=on]:bg-surface data-[state=on]:text-text',
		itemStyle,
		itemTitle,
		item,
		class: className = ''
	}: Props = $props();
</script>

<ToggleGroup.Root
	type="single"
	bind:value={() => value, (next) => next && (value = next as T)}
	aria-label={label}
	class="flex gap-1 {className}"
>
	{#each options as option (option)}
		<ToggleGroup.Item
			value={option}
			aria-label={itemLabel(option)}
			title={itemTitle?.(option)}
			{disabled}
			style={itemStyle?.(option)}
			class="h-6 flex-1 cursor-pointer rounded border transition-colors {itemClass}"
		>
			{#if item}{@render item(option, value === option)}{:else}{option}{/if}
		</ToggleGroup.Item>
	{/each}
</ToggleGroup.Root>
