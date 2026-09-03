<script lang="ts">
	import { Columns2, Diff, SplitSquareVertical } from '@lucide/svelte';
	import ConverterToolbar from '$lib/components/ConverterToolbar.svelte';
	import DocumentCanvas from '$lib/components/viewer/DocumentCanvas.svelte';
	import ViewerToolbarShell from '$lib/components/viewer/ViewerToolbarShell.svelte';
	import SegmentedControl from '$lib/components/ui/SegmentedControl.svelte';
	import type { CommitFixtureResult } from '$lib/regression/commit-runner';
	import { DocumentZoomCamera } from '$lib/viewer/document-zoom-camera.svelte';
	import CommitComparisonPage from './CommitComparisonPage.svelte';

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

	const pages = $derived(result.diff?.pages ?? []);
	const pageCount = $derived(pages.length);
	const visiblePageIndex = $derived(Math.min(Math.max(0, pageIndex), Math.max(0, pageCount - 1)));
	const zoom = new DocumentZoomCamera(() => visiblePageIndex);
	const controlsDisabled = $derived(pageCount === 0 || result.status === 'rendering');

	$effect(() => {
		if (previousFixtureId && previousFixtureId !== result.fixture.id) {
			pageIndex = 0;
			zoom.reset();
		}
		previousFixtureId = result.fixture.id;
	});

	function selectPage(nextPage: number): void {
		if (!pageCount) return;
		const selected = Math.min(pageCount - 1, Math.max(0, nextPage));
		zoom.scrollToPage(selected);
		pageIndex = selected;
	}

	function stepPage(direction: -1 | 1): void {
		selectPage(visiblePageIndex + direction);
	}
</script>

<section class="motion-surface-in flex min-w-0 flex-1 flex-col bg-canvas" aria-label="Render comparison">
	<ViewerToolbarShell label="Comparison toolbar">
		{#snippet start()}
			<div class="leading-tight">
				<strong class="block truncate text-[11px] font-[550]">{result.fixture.id}</strong>
				<span class="block truncate font-mono text-[9px] text-muted">{result.message}</span>
			</div>
		{/snippet}

		{#snippet center()}
			<ConverterToolbar
				model={{
					pageIndex: visiblePageIndex,
					pageCount,
					previewZoom: zoom.visibleZoom,
					fitPage: zoom.visiblePageFit,
					disabled: controlsDisabled
				}}
				actions={{
					onSelectPage: selectPage,
					onStepPage: stepPage,
					onSetZoom: zoom.setZoom,
					onStepZoom: zoom.stepZoom,
					onFitWidth: zoom.fitWidth,
					onFitPage: zoom.fitSelectedPage
				}}
			/>
		{/snippet}

		{#snippet end()}
			<SegmentedControl
				options={VIEW_MODES}
				bind:value={viewMode}
				label="Comparison view"
				itemLabel={(mode) => mode}
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
		{/snippet}
	</ViewerToolbarShell>

	<DocumentCanvas
		{pages}
		pageIndex={visiblePageIndex}
		{zoom}
		disabled={controlsDisabled}
		busy={result.status === 'rendering'}
		onPageChange={(nextPage) => (pageIndex = nextPage)}
		class="p-3 sm:p-5"
	>
		{#snippet page(pageDiff, index)}
			<CommitComparisonPage
				page={pageDiff}
				pageIndex={index}
				{viewMode}
				pageFit={zoom.pageFit}
				{leftLabel}
				{rightLabel}
			/>
		{/snippet}
		{#snippet empty()}
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
		{/snippet}
	</DocumentCanvas>
</section>
