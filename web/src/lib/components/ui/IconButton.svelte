<script lang="ts">
	import { mergeProps } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import Tooltip from './Tooltip.svelte';

	interface Props extends HTMLButtonAttributes {
		label: string;
		tooltip?: boolean | string;
		size?: 6 | 7 | 8 | 9;
		active?: boolean;
		tone?: 'default' | 'danger';
		children: Snippet;
	}

	let {
		label,
		tooltip = false,
		size = 6,
		active = false,
		tone = 'default',
		class: className = '',
		children,
		...rest
	}: Props = $props();

	const sizes = { 6: 'size-6', 7: 'size-7', 8: 'size-8', 9: 'size-9' } as const;
	const tones = { default: 'hover:text-text', danger: 'hover:text-negative' } as const;
	const buttonClass = $derived(
		`flex ${sizes[size]} cursor-pointer items-center justify-center rounded transition-colors hover:bg-surface disabled:cursor-default disabled:opacity-40 ${tones[tone]} ${active ? 'bg-surface text-text' : 'text-muted'} ${className}`
	);
</script>

{#snippet button(tooltipProps: Record<string, unknown>)}
	<button {...mergeProps(tooltipProps, rest)} type="button" aria-label={label} class={buttonClass}>
		{@render children()}
	</button>
{/snippet}

{#if tooltip}
	<Tooltip text={typeof tooltip === 'string' ? tooltip : label}>
		{#snippet children(props)}
			{@render button(props)}
		{/snippet}
	</Tooltip>
{:else}
	{@render button({})}
{/if}
