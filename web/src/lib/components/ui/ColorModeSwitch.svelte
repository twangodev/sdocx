<script lang="ts">
	import { Monitor, Moon, Sun } from '@lucide/svelte';
	import type { ColorMode } from '$converter/protocol';
	import SegmentedControl from './SegmentedControl.svelte';

	interface Props {
		value: ColorMode;
		disabled?: boolean;
		onChange: (mode: ColorMode) => void;
		class?: string;
	}

	let {
		value,
		disabled = false,
		onChange,
		class: className = ''
	}: Props = $props();

	const modes = ['auto', 'light', 'dark'] as const satisfies readonly ColorMode[];

	function modeLabel(mode: ColorMode): string {
		if (mode === 'auto') return 'Automatic color mode';
		if (mode === 'light') return 'Light document mode';
		return 'Dark document mode';
	}
</script>

<SegmentedControl
	options={modes}
	label="Document color mode"
	{disabled}
	itemLabel={modeLabel}
	itemTitle={modeLabel}
	bind:value={() => value, onChange}
	class="w-16 {className}"
>
	{#snippet item(mode)}
		{#if mode === 'auto'}
			<Monitor size={12} strokeWidth={1.4} />
		{:else if mode === 'light'}
			<Sun size={12} strokeWidth={1.4} />
		{:else}
			<Moon size={12} strokeWidth={1.4} />
		{/if}
	{/snippet}
</SegmentedControl>
