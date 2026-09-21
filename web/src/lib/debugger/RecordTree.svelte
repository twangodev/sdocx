<script lang="ts">
	import {
		ChevronDown,
		ChevronRight,
		LoaderCircle,
		FileText,
		Layers,
		PenLine,
		Box,
		ListPlus
	} from '@lucide/svelte';
	import RecordTree from './RecordTree.svelte';
	import type { DebugQuery, DebugRequest } from './model';
	let {
		label,
		request,
		query,
		onselect,
		selected
	}: {
		label: string;
		request: DebugRequest;
		query: DebugQuery;
		onselect: (r: DebugRequest, v: Record<string, unknown>) => void;
		selected: string;
	} = $props();
	let open = $state(false),
		busy = $state(false),
		error = $state(''),
		count = $state(100);
	let data = $state.raw<Record<string, unknown> | null>(null);
	const key = $derived(JSON.stringify(request));
	const children = $derived.by(() => {
		if (!data || !('page' in request)) return [];
		const page = request.page;
		if (request.kind === 'page')
			return (
				data.layers as { index: number; number: number; objects: number }[]
			).map((l) => ({
				label: `Layer ${l.number} · ${l.objects} objects`,
				request: { kind: 'layer', page, layer: l.index } as DebugRequest
			}));
		return (
			(data.objects ?? []) as {
				offset: number;
				type: unknown;
				children: number;
			}[]
		).map((o) => ({
			label: `${typeof o.type === 'string' ? o.type : JSON.stringify(o.type)} @ 0x${o.offset.toString(16)}${o.children ? ` · ${o.children} children` : ''}`,
			request: { kind: 'object', page, offset: o.offset } as DebugRequest
		}));
	});
	async function choose() {
		busy = true;
		error = '';
		try {
			const result = await query<Record<string, unknown>>(request);
			onselect(request, result);
			// Retain child summaries, not every previously inspected sample array.
			data = { layers: result.layers, objects: result.objects };
			open = !open;
		} catch (e) {
			error = String(e);
		} finally {
			busy = false;
		}
	}
</script>

<div class="record">
	<button
		class:chosen={selected === key}
		onclick={choose}
		aria-expanded={open}
		title={label}
		>{#if busy}<LoaderCircle
				size={12}
				class="animate-spin"
				aria-hidden="true"
			/>
		{:else if open}<ChevronDown
				size={12}
				aria-hidden="true"
			/>{:else}<ChevronRight size={12} aria-hidden="true" />{/if}
		{#if request.kind === 'page'}<FileText size={13} aria-hidden="true" />
		{:else if request.kind === 'layer'}<Layers size={13} aria-hidden="true" />
		{:else if label.startsWith('Stroke')}<PenLine
				size={13}
				aria-hidden="true"
			/>
		{:else}<Box size={13} aria-hidden="true" />{/if}
		<span>{label}</span></button
	>
	{#if error}<p role="alert">{error}</p>{/if}
	{#if open && children.length}
		<div class="children">
			{#each children.slice(0, count) as child (JSON.stringify(child.request))}<RecordTree
					{...child}
					{query}
					{onselect}
					{selected}
				/>{/each}
			{#if children.length > count}<button onclick={() => (count += 100)}
					><ListPlus size={13} aria-hidden="true" />Show next 100</button
				>{/if}
		</div>
	{/if}
</div>

<style>
	button {
		display: flex;
		align-items: center;
		gap: 5px;
		text-align: left;
		width: 100%;
		padding: 5px 6px;
		border-radius: 4px;
		cursor: pointer;
		overflow-wrap: anywhere;
	}
	button :global(svg) {
		flex-shrink: 0;
	}
	button:hover,
	.chosen {
		background: color-mix(
			in srgb,
			var(--color-accent, #497add) 17%,
			transparent
		);
	}
	.children {
		padding-left: 12px;
		border-left: 1px solid var(--color-subtle, #ddd);
		margin-left: 8px;
	}
	p {
		color: #c55;
	}
</style>
