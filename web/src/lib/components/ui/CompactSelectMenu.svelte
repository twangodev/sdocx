<script lang="ts" generics="A">
	import { ChevronDown } from '@lucide/svelte';
	import type { MenuLeaf } from '$lib/menu';
	import DropdownMenu from './DropdownMenu.svelte';

	interface Props {
		label: string;
		value: string;
		items: MenuLeaf<A>[];
		onAction: (action: A) => void;
		align?: 'start' | 'center' | 'end';
		disabled?: boolean;
		chevron?: boolean;
		class?: string;
	}

	let {
		label,
		value,
		items,
		onAction,
		align = 'start',
		disabled = false,
		chevron = true,
		class: className = ''
	}: Props = $props();
</script>

<DropdownMenu {items} {onAction} {align} size="compact">
	{#snippet children({ props })}
		<button
			{...props}
			type="button"
			aria-label={label}
			{disabled}
			class="compact-select flex h-6 cursor-pointer items-center gap-1 rounded px-1.5 font-mono text-[11px] tabular-nums text-text outline-none transition-[background-color,color,transform] duration-150 ease-out hover:bg-surface disabled:cursor-default disabled:opacity-40 {className}"
		>
			<span class="min-w-0 flex-1 truncate">{value}</span>
			{#if chevron}<ChevronDown size={10} strokeWidth={1.5} class="shrink-0 text-muted" />{/if}
		</button>
	{/snippet}
</DropdownMenu>

<style>
	.compact-select :global(svg) {
		transition: transform var(--motion-fast) var(--ease-out);
	}

	.compact-select[data-state='open'] :global(svg) {
		transform: rotate(180deg);
	}
</style>
