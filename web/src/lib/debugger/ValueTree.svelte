<script lang="ts">
	import { ChevronDown, ChevronRight, ListPlus } from '@lucide/svelte';
	import ValueTree from './ValueTree.svelte';
	let { value, label = 'Properties' }: { value: unknown; label?: string } =
		$props();
	let open = $state(false);
	let count = $state(100);
	const entries = $derived(
		value && typeof value === 'object' ? Object.entries(value) : null
	);
</script>

{#if entries}
	<div class="value-tree">
		<button class="branch" onclick={() => (open = !open)} aria-expanded={open}
			>{#if open}<ChevronDown
					size={12}
					aria-hidden="true"
				/>{:else}<ChevronRight size={12} aria-hidden="true" />{/if}
			{label} <span>({entries.length})</span></button
		>
		{#if open}
			<div class="children">
				{#each entries.slice(0, count) as [key, item] (key)}<ValueTree
						value={item}
						label={key}
					/>{/each}
				{#if entries.length > count}<button onclick={() => (count += 100)}
						><ListPlus size={13} aria-hidden="true" />Show next 100</button
					>{/if}
			</div>
		{/if}
	</div>
{:else}
	<div class="leaf">
		<span>{label}:</span> <code>{value === null ? 'null' : String(value)}</code>
	</div>
{/if}

<style>
	.children {
		padding-left: 12px;
		border-left: 1px solid var(--color-subtle, #ddd);
		margin-left: 4px;
	}
	.branch {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 3px 0;
		text-align: left;
		cursor: pointer;
	}
	.branch span,
	.leaf span {
		opacity: 0.65;
	}
	.leaf {
		padding: 3px 0;
		overflow-wrap: anywhere;
	}
	code {
		white-space: pre-wrap;
		font-size: 11px;
	}
</style>
