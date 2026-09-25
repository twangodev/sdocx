<script lang="ts">
	import { Checkbox } from 'bits-ui';
	import { Check, Minus } from '@lucide/svelte';
	let { label, checked, indeterminate = false, disabled = false, onCheckedChange }: {
		label: string;
		checked: boolean;
		indeterminate?: boolean;
		disabled?: boolean;
		onCheckedChange: (checked: boolean) => void;
	} = $props();
</script>

<Checkbox.Root {checked} {indeterminate} {disabled} {onCheckedChange} aria-label={label} class="group grid size-7 shrink-0 cursor-pointer place-items-center rounded hover:bg-surface disabled:cursor-default disabled:opacity-40">
	<span class="grid size-3.5 place-items-center rounded-[3px] border border-control-edge bg-bg text-bg group-data-[state=checked]:border-text group-data-[state=checked]:bg-text group-data-[state=indeterminate]:border-text group-data-[state=indeterminate]:bg-text">
		{#if indeterminate}<Minus size={10} strokeWidth={2.5} />{:else if checked}<Check size={10} strokeWidth={2.5} />{/if}
	</span>
</Checkbox.Root>
