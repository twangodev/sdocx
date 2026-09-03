<script lang="ts">
	import CompactSelectMenu from '$lib/components/ui/CompactSelectMenu.svelte';
	import SegmentedControl from '$lib/components/ui/SegmentedControl.svelte';
	import type { MenuLeaf } from '$lib/menu';
	import type { ComparisonPage } from '$lib/regression/pdf';
	import { clampPageIndex } from '$lib/regression/view-model';
	import ComparisonMetrics from './ComparisonMetrics.svelte';
	import SectionHeading from './SectionHeading.svelte';

	type ViewMode = 'side-by-side' | 'opacity' | 'swipe' | 'heatmap';
	const VIEW_MODES: readonly ViewMode[] = ['side-by-side', 'opacity', 'swipe', 'heatmap'];

	let { fixtureId, comparisons }: { fixtureId: string; comparisons: ComparisonPage[] } = $props();
	let selectedPageIndex = $state(0);
	let viewMode = $state<ViewMode>('side-by-side');
	let opacity = $state(50);
	let swipe = $state(50);
	let previousFixtureId = '';

	const pageIndex = $derived(clampPageIndex(selectedPageIndex, comparisons.length));
	const selectedPage = $derived(comparisons[pageIndex]);
	const pageMenu = $derived<MenuLeaf<number>[]>(
		comparisons.map((page) => ({
			kind: 'action',
			label: `page ${page.pageIndex + 1}`,
			action: page.pageIndex,
			checked: page.pageIndex === pageIndex
		}))
	);

	$effect(() => {
		if (previousFixtureId && fixtureId !== previousFixtureId) selectedPageIndex = 0;
		previousFixtureId = fixtureId;
	});
</script>

<section class="border-t border-subtle px-5 py-5.5" aria-labelledby="comparison-heading">
	<SectionHeading
		id="comparison-heading"
		title="Visual comparison"
		description="Normalized browser render against the verified reference PDF."
		class="max-[650px]:block"
	>
		{#snippet aside()}
			{#if comparisons.length > 0}
				<div class="flex items-center gap-2 text-muted max-[720px]:mt-3">
					<span class="mono-label">Page</span>
					<CompactSelectMenu
						label="Choose comparison page"
						value={String(pageIndex + 1)}
						items={pageMenu}
						onAction={(page) => (selectedPageIndex = page)}
						align="end"
						class="min-w-12"
					/>
				</div>
			{/if}
		{/snippet}
	</SectionHeading>

	{#if !selectedPage}
		<p
			class="m-0 rounded-[0.45rem] border border-dashed border-subtle p-8 text-center text-[0.8rem] text-muted"
		>
			Reference and generated pages appear here after the run.
		</p>
	{:else}
		<div class="mb-3 flex flex-wrap items-center gap-2">
			<SegmentedControl
				options={VIEW_MODES}
				bind:value={viewMode}
				label="Comparison mode"
				itemLabel={(mode) => mode.replace('-', ' ')}
				class="h-7"
			/>
			<a
				href={selectedPage.actualSvgUrl}
				download={`sdocx-page-${selectedPage.pageIndex + 1}.svg`}
				class="ml-auto inline-flex min-h-7 items-center justify-center rounded border border-subtle bg-surface px-2 font-mono text-[0.63rem] text-muted no-underline transition-[border-color,background-color,color,transform] duration-[var(--motion-fast)] ease-[var(--ease-standard)] hover:border-accent/65 hover:text-accent"
			>
				Download SVG
			</a
			>
		</div>

		{#if viewMode === 'opacity'}
			<label
				class="motion-surface-in my-3 grid grid-cols-[auto_auto_1fr] items-center gap-2.5 font-mono text-[0.65rem] text-muted"
			>
				Generated opacity <strong>{opacity}%</strong>
				<input class="w-full accent-accent" type="range" min="0" max="100" bind:value={opacity} />
			</label>
		{:else if viewMode === 'swipe'}
			<label
				class="motion-surface-in my-3 grid grid-cols-[auto_auto_1fr] items-center gap-2.5 font-mono text-[0.65rem] text-muted"
			>
				Reveal generated <strong>{swipe}%</strong>
				<input class="w-full accent-accent" type="range" min="0" max="100" bind:value={swipe} />
			</label>
		{/if}

		{#if viewMode === 'side-by-side'}
			<div class="motion-fade-in grid grid-cols-2 gap-2.5 max-[650px]:grid-cols-1">
				<figure class="m-0 min-w-0">
					<img
						class="block h-auto w-full border border-subtle bg-white"
						src={selectedPage.actualRasterUrl}
						alt="Generated SDOCX page"
					/>
					<figcaption class="px-0.5 py-2 font-mono text-[0.62rem] text-muted">
						Generated SVG
					</figcaption>
				</figure>
				<figure class="m-0 min-w-0">
					<img
						class="block h-auto w-full border border-subtle bg-white"
						src={selectedPage.referenceUrl}
						alt="Samsung Notes reference PDF page"
					/>
					<figcaption class="px-0.5 py-2 font-mono text-[0.62rem] text-muted">
						Reference PDF
					</figcaption>
				</figure>
			</div>
		{:else if viewMode === 'heatmap'}
			<figure class="motion-fade-in mx-auto my-0 max-w-[52rem] min-w-0">
				<img
					class="block h-auto w-full border border-subtle bg-[#111]"
					src={selectedPage.heatmapUrl}
					alt="Pixel difference heatmap"
				/>
				<figcaption class="px-0.5 py-2 font-mono text-[0.62rem] text-muted">
					Brighter red pixels have larger RGB differences.
				</figcaption>
			</figure>
		{:else}
			<div class="motion-fade-in relative mx-auto max-w-[52rem] overflow-hidden">
				<img
					class="block h-auto w-full border border-subtle bg-white"
					src={selectedPage.referenceUrl}
					alt="Samsung Notes reference PDF page"
				/>
				<img
					class="absolute inset-0 block h-auto w-full border border-subtle bg-white"
					src={selectedPage.actualRasterUrl}
					alt="Generated SDOCX page"
					style={viewMode === 'opacity'
						? `opacity: ${opacity / 100}`
						: `clip-path: inset(0 ${100 - swipe}% 0 0)`}
				/>
				{#if viewMode === 'swipe'}
					<span
						class="absolute inset-y-0 w-0.5 -translate-x-px bg-accent shadow-[0_0_0_1px_white]"
						style={`left: ${swipe}%`}
					></span>
				{/if}
			</div>
		{/if}

		<ComparisonMetrics metrics={selectedPage.metrics} />
	{/if}
</section>
