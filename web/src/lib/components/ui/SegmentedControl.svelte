<script lang="ts" generics="T extends string">
	import { ToggleGroup } from 'bits-ui';
	import type { Snippet } from 'svelte';

	interface Props {
		options: readonly T[];
		value: T;
		label: string;
		disabled?: boolean;
		itemLabel?: (option: T) => string;
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
		itemTitle,
		item,
		class: className = ''
	}: Props = $props();
</script>

<ToggleGroup.Root
	type="single"
	bind:value={() => value, (next) => next && (value = next as T)}
	aria-label={label}
	class="inline-grid h-6 grid-flow-col auto-cols-fr items-center rounded border border-subtle bg-surface p-px {className}"
>
	{#each options as option (option)}
		<ToggleGroup.Item
			value={option}
			aria-label={itemLabel(option)}
			title={itemTitle?.(option)}
			{disabled}
			class="flex h-5 min-w-0 cursor-pointer items-center justify-center rounded-[3px] border-0 bg-transparent p-0 leading-none text-muted outline-none transition-[background-color,color,box-shadow] hover:text-text disabled:cursor-default disabled:opacity-40 data-[state=on]:bg-raised data-[state=on]:text-text data-[state=on]:shadow-sm"
		>
			{#if item}{@render item(option, value === option)}{:else}{option}{/if}
		</ToggleGroup.Item>
	{/each}
</ToggleGroup.Root>
