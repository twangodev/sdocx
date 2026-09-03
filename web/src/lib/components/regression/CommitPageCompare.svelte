<script lang="ts">
	import { Columns2, Diff, SplitSquareVertical } from '@lucide/svelte';
	import PageNumberInput from '$lib/components/ui/PageNumberInput.svelte';
	import SegmentedControl from '$lib/components/ui/SegmentedControl.svelte';
	import type { CommitFixtureResult } from '$lib/regression/commit-runner';
	import CommitSwipeCompare from './CommitSwipeCompare.svelte';

	type ViewMode = 'split' | 'swipe' | 'difference';
	const VIEW_MODES: readonly ViewMode[] = ['split', 'swipe', 'difference'];

	interface Props {
		result: CommitFixtureResult;
		leftLabel: string;
		rightLabel: string;
		onRun: () => void;
	}

	let { result, leftLabel, rightLabel, onRun }: Props = $props();
	let viewMode = $state<ViewMode>('split');
	let pageIndex = $state(0);
	let previousFixtureId = '';

	const pageCount = $derived(result.diff?.pages.length ?? 0);
	const visiblePageIndex = $derived(Math.min(Math.max(0, pageIndex), Math.max(0, pageCount - 1)));
	const page = $derived(result.diff?.pages[visiblePageIndex]);
	const aspectRatio = $derived(
		page?.right
			? `${page.right.width} / ${page.right.height}`
			: page?.left
				? `${page.left.width} / ${page.left.height}`
				: '1 / 1.414'
	);

	$effect(() => {
		if (previousFixtureId && previousFixtureId !== result.fixture.id) pageIndex = 0;
		previousFixtureId = result.fixture.id;
	});

</script>

<section class="motion-surface-in flex min-w-0 flex-1 flex-col bg-canvas" aria-label="Render comparison">
	<div class="flex h-9 shrink-0 items-center justify-between border-b border-subtle bg-bg px-3">
		<div class="min-w-0">
			<span class="block truncate font-mono text-[11px] text-text">{result.fixture.id}</span>
		</div>

		{#if page}
			<SegmentedControl
				options={VIEW_MODES}
				bind:value={viewMode}
				label="Comparison view"
				itemLabel={(mode) => mode}
				class="absolute left-1/2 -translate-x-1/2"
			>
				{#snippet item(mode)}
					{#if mode === 'split'}
						<Columns2 size={12} strokeWidth={1.5} />
					{:else if mode === 'swipe'}
						<SplitSquareVertical size={12} strokeWidth={1.5} />
					{:else}
						<Diff size={12} strokeWidth={1.5} />
					{/if}
				{/snippet}
			</SegmentedControl>
			<PageNumberInput
				pageIndex={visiblePageIndex}
				{pageCount}
				onSelect={(nextPage) => (pageIndex = nextPage)}
			/>
		{:else}
			<span class="font-mono text-[10px] text-muted">{result.message}</span>
		{/if}
	</div>

	<div class="min-h-0 flex-1 overflow-auto p-3 sm:p-5">
		{#if !page}
			<div class="flex min-h-full items-center justify-center">
				<div class="max-w-xs text-center">
					{#if result.status === 'idle'}
						<p class="m-0 text-[12px] text-text">Ready to compare</p>
						<p class="mt-1.5 mb-4 text-[11px] leading-5 text-muted">
							Render the compatibility corpus with both commits in this browser.
						</p>
						<button
							type="button"
							aria-label="Run comparison"
							class="h-8 cursor-pointer rounded bg-text px-3 text-[11px] font-medium text-bg transition-opacity hover:opacity-80"
							onclick={onRun}>run comparison</button
						>
					{:else if result.status === 'failed'}
						<p class="m-0 text-[12px] text-negative">{result.error}</p>
					{:else}
						<span
							class="mx-auto mb-3 block size-4 animate-spin rounded-full border border-subtle border-t-accent"
							aria-hidden="true"
						></span>
						<p class="m-0 font-mono text-[11px] text-muted">{result.message}</p>
					{/if}
				</div>
			</div>
		{:else if viewMode === 'split'}
			<div class="mx-auto grid max-w-[92rem] grid-cols-2 items-start gap-3 max-[720px]:grid-cols-1">
				<figure class="m-0 min-w-0">
					<figcaption class="mb-1.5 flex items-center gap-1.5 font-mono text-[10px] text-muted">
						<span class="size-1.5 rounded-full bg-muted"></span>{leftLabel}
					</figcaption>
					{#if page.leftUrl}
						<img
							src={page.leftUrl}
							alt={`${leftLabel} render of page ${visiblePageIndex + 1}`}
							class="block h-auto w-full border border-black/15 bg-white shadow-[0_6px_20px_rgb(0_0_0_/_0.12)]"
						/>
					{:else}
						<div class="grid w-full place-items-center border border-dashed border-subtle text-[11px] text-muted" style:aspect-ratio={aspectRatio}>missing</div>
					{/if}
				</figure>
				<figure class="m-0 min-w-0">
					<figcaption class="mb-1.5 flex items-center gap-1.5 font-mono text-[10px] text-muted">
						<span class="size-1.5 rounded-full bg-accent"></span>{rightLabel}
					</figcaption>
					{#if page.rightUrl}
						<img
							src={page.rightUrl}
							alt={`${rightLabel} render of page ${visiblePageIndex + 1}`}
							class="block h-auto w-full border border-black/15 bg-white shadow-[0_6px_20px_rgb(0_0_0_/_0.12)]"
						/>
					{:else}
						<div class="grid w-full place-items-center border border-dashed border-subtle text-[11px] text-muted" style:aspect-ratio={aspectRatio}>missing</div>
					{/if}
				</figure>
			</div>
		{:else if viewMode === 'swipe'}
			<CommitSwipeCompare {page} {aspectRatio} {leftLabel} {rightLabel} />
		{:else}
			<div
				class="relative mx-auto w-full max-w-[72rem] overflow-hidden border border-black/15 bg-black shadow-[0_6px_20px_rgb(0_0_0_/_0.12)]"
				style:aspect-ratio={aspectRatio}
			>
				{#if page.leftUrl}
					<img class="absolute inset-0 size-full" src={page.leftUrl} alt="" />
				{/if}
				{#if page.rightUrl}
					<img class="absolute inset-0 size-full mix-blend-difference" src={page.rightUrl} alt="" />
				{/if}
			</div>
		{/if}
	</div>
</section>
