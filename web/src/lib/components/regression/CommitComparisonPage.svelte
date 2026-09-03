<script lang="ts">
	import type { CommitPageDiff } from '$lib/regression/commit-compare';
	import CommitSwipeCompare from './CommitSwipeCompare.svelte';

	type ViewMode = 'split' | 'swipe' | 'difference';

	interface Props {
		page: CommitPageDiff;
		pageIndex: number;
		viewMode: ViewMode;
		pageFit: boolean;
		leftLabel: string;
		rightLabel: string;
	}

	let { page, pageIndex, viewMode, pageFit, leftLabel, rightLabel }: Props = $props();
	const dimensions = $derived(page.right ?? page.left);
	const aspectRatio = $derived(dimensions ? dimensions.width / dimensions.height : 1 / 1.414);
	const splitAspectRatio = $derived(aspectRatio * 2);
	const pageWidth = $derived(
		pageFit ? `min(100%, calc((100svh - 10rem) * ${aspectRatio}))` : '100%'
	);
	const splitWidth = $derived(
		pageFit ? `min(100%, calc((100svh - 10rem) * ${splitAspectRatio}))` : '100%'
	);
</script>

{#if viewMode === 'split'}
	<div
		data-page-zoom-target
		class="grid h-auto max-w-full grid-cols-2 gap-2"
		style:aspect-ratio={splitAspectRatio}
		style:width={splitWidth}
	>
		<div class="relative min-h-0 min-w-0 overflow-hidden border border-black/15 bg-white shadow-[0_6px_20px_rgb(0_0_0_/_0.12)]">
			<span class="pointer-events-none absolute top-2 left-2 z-10 rounded bg-black/65 px-1.5 py-1 font-mono text-[10px] text-white">
				{leftLabel}
			</span>
			{#if page.leftUrl}
				<img
					src={page.leftUrl}
					alt={`${leftLabel} render of page ${pageIndex + 1}`}
					class="block size-full object-contain"
				/>
			{:else}
				<div class="grid size-full place-items-center text-[11px] text-muted">missing</div>
			{/if}
		</div>
		<div class="relative min-h-0 min-w-0 overflow-hidden border border-black/15 bg-white shadow-[0_6px_20px_rgb(0_0_0_/_0.12)]">
			<span class="pointer-events-none absolute top-2 right-2 z-10 rounded bg-black/65 px-1.5 py-1 font-mono text-[10px] text-white">
				{rightLabel}
			</span>
			{#if page.rightUrl}
				<img
					src={page.rightUrl}
					alt={`${rightLabel} render of page ${pageIndex + 1}`}
					class="block size-full object-contain"
				/>
			{:else}
				<div class="grid size-full place-items-center text-[11px] text-muted">missing</div>
			{/if}
		</div>
	</div>
{:else if viewMode === 'swipe'}
	<CommitSwipeCompare {page} {aspectRatio} {leftLabel} {rightLabel} width={pageWidth} />
{:else}
	<div
		data-page-zoom-target
		class="relative h-auto max-w-full overflow-hidden border border-black/15 bg-black shadow-[0_6px_20px_rgb(0_0_0_/_0.12)]"
		style:aspect-ratio={aspectRatio}
		style:width={pageWidth}
	>
		{#if page.leftUrl}
			<img class="absolute inset-0 size-full" src={page.leftUrl} alt="" />
		{/if}
		{#if page.rightUrl}
			<img class="absolute inset-0 size-full mix-blend-difference" src={page.rightUrl} alt="" />
		{/if}
	</div>
{/if}
