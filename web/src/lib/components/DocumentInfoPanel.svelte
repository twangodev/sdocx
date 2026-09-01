<script lang="ts">
	import { Check } from '@lucide/svelte';
	import type { InspectionView } from '$converter/view-model';

	let {
		pageCount,
		details,
		class: className = ''
	}: { pageCount: number; details: InspectionView | null; class?: string } = $props();
</script>

<aside
	class="min-w-0 overflow-auto border-r border-subtle bg-bg p-2.5 {className}"
	aria-label="Document information"
>
	<span class="text-[10px] font-semibold text-muted">document info</span>
	<dl class="mt-1.5">
		<div class="grid grid-cols-[0.7fr_1fr] gap-1.5 border-b border-subtle py-1.5 text-[11px]">
			<dt class="text-muted">Pages</dt><dd class="m-0 break-words text-right">{pageCount}</dd>
		</div>
		<div class="grid grid-cols-[0.7fr_1fr] gap-1.5 border-b border-subtle py-1.5 text-[11px]">
			<dt class="text-muted">Format</dt><dd class="m-0 break-words text-right">{details?.formatVersion ?? 'Unknown'}</dd>
		</div>
		<div class="grid grid-cols-[0.7fr_1fr] gap-1.5 border-b border-subtle py-1.5 text-[11px]">
			<dt class="text-muted">Page size</dt><dd class="m-0 break-words text-right">{details?.dimensions ?? 'Varies'}</dd>
		</div>
		<div class="grid grid-cols-[0.7fr_1fr] gap-1.5 border-b border-subtle py-1.5 text-[11px]">
			<dt class="text-muted">Media</dt><dd class="m-0 break-words text-right">{details?.mediaCount ?? 0}</dd>
		</div>
		{#if details?.created}
			<div class="grid grid-cols-[0.7fr_1fr] gap-1.5 border-b border-subtle py-1.5 text-[11px]">
				<dt class="text-muted">Created</dt><dd class="m-0 break-words text-right">{details.created}</dd>
			</div>
		{/if}
		{#if details?.modified}
			<div class="grid grid-cols-[0.7fr_1fr] gap-1.5 border-b border-subtle py-1.5 text-[11px]">
				<dt class="text-muted">Modified</dt><dd class="m-0 break-words text-right">{details.modified}</dd>
			</div>
		{/if}
	</dl>

	<div class="mt-4">
		<span class="text-[11px] font-semibold text-muted">diagnostics</span>
		{#if details?.diagnostics.length}
			<ul class="mt-2 flex list-none flex-col gap-2 p-0">
				{#each details.diagnostics as diagnostic}
					<li class="flex flex-col gap-0.5 border-l-2 border-[#c9952f] pl-2.5 text-[11px]">
						<strong>{diagnostic.code}</strong>
						<span class="text-muted">{diagnostic.message}</span>
						{#if diagnostic.entry}<code class="text-muted">{diagnostic.entry}</code>{/if}
					</li>
				{/each}
			</ul>
		{:else}
			<p class="mt-2 flex items-center gap-1.5 font-mono text-[11px] text-muted">
				<Check class="text-positive" size={13} strokeWidth={2.5} aria-hidden="true" />
				No parser warnings
			</p>
		{/if}
	</div>
</aside>
