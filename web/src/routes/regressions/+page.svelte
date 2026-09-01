<script lang="ts">
	import { Check, X } from '@lucide/svelte';
	import { onDestroy } from 'svelte';
	import CompactSelectMenu from '$lib/components/ui/CompactSelectMenu.svelte';
	import type { MenuLeaf } from '$lib/menu';
	import { CORPUS_FIXTURES } from '../../lib/regression/manifest';
	import { disposeComparisonPages } from '../../lib/regression/pdf';
	import {
		createRegressionReport,
		downloadRegressionArchive,
		downloadReport,
		type RegressionReport
	} from '../../lib/regression/report';
	import {
		RegressionSuiteRunner,
		type FixtureRunResult,
		type LocalFixtureAssets,
		type SuiteExecution
	} from '../../lib/regression/runner';
	import { clampPageIndex, statusTone } from './page-model';

	type ViewMode = 'side-by-side' | 'opacity' | 'swipe' | 'heatmap';
	type LocalSelection = Partial<LocalFixtureAssets>;

	const runner = new RegressionSuiteRunner();
	let results: FixtureRunResult[] = CORPUS_FIXTURES.map((fixture) => ({
		fixture,
		status: 'queued',
		message: 'Ready',
		checks: [],
		comparisons: []
	}));
	let execution: SuiteExecution | undefined;
	let report: RegressionReport | undefined;
	let running = false;
	let packaging = false;
	let selectedId = CORPUS_FIXTURES[0]?.id ?? '';
	let selectedPageIndex = 0;
	let viewMode: ViewMode = 'side-by-side';
	let opacity = 50;
	let swipe = 50;
	let selectedResult: FixtureRunResult | undefined;
	let selectedPage: FixtureRunResult['comparisons'][number] | undefined;
	let selectedLocal: LocalSelection | undefined;
	let localSelections: Record<string, LocalSelection> = {};
	let uiError = '';

	$: selectedResult = results.find((result) => result.fixture.id === selectedId) ?? results[0];
	$: selectedLocal = selectedResult ? localSelections[selectedResult.fixture.id] : undefined;
	$: selectedPageIndex = clampPageIndex(selectedPageIndex, selectedResult?.comparisons.length ?? 0);
	$: selectedPage = selectedResult?.comparisons[selectedPageIndex];

	function replaceResults(updated: FixtureRunResult[], merge: boolean): void {
		const replacements = new Map(updated.map((result) => [result.fixture.id, result]));
		const next = merge
			? results.map((result) => replacements.get(result.fixture.id) ?? result)
			: updated;
		const retainedPages = new Set(next.flatMap((result) => result.comparisons));
		disposeComparisonPages(
			results.flatMap((result) => result.comparisons).filter((page) => !retainedPages.has(page))
		);
		results = next;
	}

	function localAssets(): ReadonlyMap<string, LocalFixtureAssets> {
		const assets = new Map<string, LocalFixtureAssets>();
		for (const [id, selection] of Object.entries(localSelections)) {
			if (selection.sdocx && selection.referencePdf) {
				assets.set(id, { sdocx: selection.sdocx, referencePdf: selection.referencePdf });
			}
		}
		return assets;
	}

	function incompleteLocalSelection(fixtureIds: Set<string>): string | undefined {
		for (const [id, selection] of Object.entries(localSelections)) {
			if (!fixtureIds.has(id)) continue;
			if (Boolean(selection.sdocx) !== Boolean(selection.referencePdf)) return id;
		}
		return undefined;
	}

	async function runFixtures(fixtures: typeof CORPUS_FIXTURES, merge: boolean): Promise<void> {
		if (running || fixtures.length === 0) return;
		const incomplete = incompleteLocalSelection(new Set(fixtures.map((fixture) => fixture.id)));
		if (incomplete) {
			uiError = `${incomplete}: choose both the .sdocx and reference PDF, or clear the local files.`;
			return;
		}

		running = true;
		uiError = '';
		execution = undefined;
		report = undefined;
		selectedPageIndex = 0;
		try {
				execution = await runner.run(
				fixtures,
				(updated) => {
					replaceResults(updated, merge);
				},
				{ localAssets: localAssets() }
			);
			replaceResults(execution.results, merge);
			report = createRegressionReport(execution);
		} catch (cause) {
			uiError = cause instanceof Error ? cause.message : 'The regression run could not start.';
		} finally {
			running = false;
		}
	}

	function runAll(): void {
		void runFixtures(CORPUS_FIXTURES, false);
	}

	function runSelected(): void {
		if (selectedResult) void runFixtures([selectedResult.fixture], true);
	}

	function cancel(): void {
		runner.cancel();
	}

	function selectFixture(id: string): void {
		selectedId = id;
		selectedPageIndex = 0;
		uiError = '';
	}

	function comparisonPageMenu(): MenuLeaf<number>[] {
		return (selectedResult?.comparisons ?? []).map((page) => ({
			kind: 'action',
			label: `page ${page.pageIndex + 1}`,
			action: page.pageIndex,
			checked: page.pageIndex === selectedPageIndex
		}));
	}

	function chooseLocalFile(kind: keyof LocalFixtureAssets, event: Event): void {
		if (!selectedResult) return;
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (!file) return;

		const id = selectedResult.fixture.id;
		localSelections = {
			...localSelections,
			[id]: { ...localSelections[id], [kind]: file }
		};
		uiError = '';
	}

	function clearLocalFiles(): void {
		if (!selectedResult) return;
		const { [selectedResult.fixture.id]: _removed, ...remaining } = localSelections;
		localSelections = remaining;
		uiError = '';
	}

	async function downloadZip(): Promise<void> {
		if (!report || !execution || packaging) return;
		packaging = true;
		uiError = '';
		try {
			await downloadRegressionArchive(report, execution);
		} catch (cause) {
			uiError = cause instanceof Error ? cause.message : 'The ZIP report could not be created.';
		} finally {
			packaging = false;
		}
	}

	onDestroy(() => {
		runner.cancel();
		disposeComparisonPages(results.flatMap((result) => result.comparisons));
	});
</script>

<svelte:head>
	<title>sdocx — regression lab</title>
	<meta
		name="description"
		content="Run the locked SDOCX compatibility corpus locally and compare browser output with Samsung Notes reference PDFs."
	/>
</svelte:head>

<section class="regression-page" aria-labelledby="regression-title">
	<header class="page-heading">
		<div class="heading-copy">
			<h1 id="regression-title">Regression lab</h1>
			<p>
				Run the compatibility corpus and compare generated pages with Samsung Notes reference PDFs.
			</p>
		</div>
		<div class="actions">
			<button class="control" type="button" onclick={runSelected} disabled={running || !selectedResult}>
				Run selected
			</button>
			<button class="control primary" type="button" onclick={runAll} disabled={running}>
				{running ? 'Running…' : 'Run all'}
			</button>
			{#if running}
				<button class="control" type="button" onclick={cancel}>Cancel</button>
			{/if}
			{#if report}
				<button class="control" type="button" onclick={() => downloadReport(report!, 'json')}>
					JSON
				</button>
				<button class="control" type="button" onclick={() => downloadReport(report!, 'html')}>
					HTML
				</button>
				<button class="control" type="button" onclick={downloadZip} disabled={packaging}>
					{packaging ? 'Packing…' : 'ZIP + images'}
				</button>
			{/if}
		</div>
	</header>

	{#if uiError}
		<div class="error-banner" role="alert"><strong>Could not continue.</strong> {uiError}</div>
	{/if}

	<div class="workspace">
		<aside class="fixtures" aria-label="Compatibility fixtures">
			<div class="panel-heading">
				<h2>Fixtures</h2>
				<span>{results.length}</span>
			</div>

			{#each results as result}
				<button
					type="button"
					class:chosen={result.fixture.id === selectedResult?.fixture.id}
					class="fixture"
					onclick={() => selectFixture(result.fixture.id)}
				>
					<span class="fixture-top">
						<strong>{result.fixture.id}</strong>
						<span class="status mono-label" data-tone={statusTone(result.status)}>{result.status}</span>
					</span>
					<span class="message">{result.message}</span>
					<span class="expectation">
						{result.fixture.storedPages} stored · {result.fixture.visiblePages} visible pages
					</span>
				</button>
			{/each}
		</aside>

		<section class="detail">
			{#if selectedResult}
				<div class="panel-heading detail-heading">
					<h2>{selectedResult.fixture.id}</h2>
					{#if selectedResult.durationMs !== undefined}
						<span class="duration mono-label">{(selectedResult.durationMs / 1000).toFixed(1)} s</span>
					{/if}
				</div>

				{#if selectedResult.error}
					<div class="fixture-error"><strong>Run did not pass.</strong> {selectedResult.error}</div>
				{/if}

				<section class="local-source" aria-labelledby="local-source-heading">
					<div class="section-heading">
						<div>
							<h3 id="local-source-heading">Local fixture fallback</h3>
							<p>Use matching files from this device if the dataset cannot be reached.</p>
						</div>
						{#if selectedLocal?.sdocx || selectedLocal?.referencePdf}
							<button class="text-button" type="button" onclick={clearLocalFiles}>Clear files</button>
						{/if}
					</div>
					<div class="local-files">
						<label class="file-control">
							<input
								type="file"
								accept=".sdocx,application/zip"
								onchange={(event) => chooseLocalFile('sdocx', event)}
							/>
							<span class="mono-label">SDOCX</span>
							<strong>{selectedLocal?.sdocx?.name ?? selectedResult.fixture.sdocx}</strong>
							<small>{selectedLocal?.sdocx ? 'Local file selected' : 'Choose local file'}</small>
						</label>
						<label class="file-control">
							<input
								type="file"
								accept=".pdf,application/pdf"
								onchange={(event) => chooseLocalFile('referencePdf', event)}
							/>
							<span class="mono-label">Reference PDF</span>
							<strong>{selectedLocal?.referencePdf?.name ?? selectedResult.fixture.referencePdf}</strong>
							<small>{selectedLocal?.referencePdf ? 'Local file selected' : 'Choose local file'}</small>
						</label>
					</div>
					<p class="hash-note">
						Local files must match the manifest SHA-256 values. They never leave this tab.
					</p>
				</section>

				<section class="checks" aria-labelledby="checks-heading">
					<div class="section-heading">
						<div>
							<h3 id="checks-heading">Structural checks</h3>
							<p>Exact assertions mirrored from the Rust external-corpus test.</p>
						</div>
						{#if selectedResult.checks.length > 0}
							<strong class:failed={selectedResult.checks.some((check) => !check.passed)}>
								{selectedResult.checks.filter((check) => check.passed).length}/{selectedResult.checks.length}
							</strong>
						{/if}
					</div>

					{#if selectedResult.checks.length === 0}
						<p class="empty">Run this fixture to inspect its structure.</p>
					{:else}
						<div class="check-grid">
							{#each selectedResult.checks as item}
								<div class:check-failed={!item.passed} class="check-row">
									<span class="check-icon" aria-hidden="true">
										{#if item.passed}
											<Check size={11} strokeWidth={2.5} />
										{:else}
											<X size={11} strokeWidth={2.5} />
										{/if}
									</span>
									<div>
										<strong>{item.label}</strong>
										<small>Expected {String(item.expected)} · got {String(item.actual)}</small>
									</div>
								</div>
							{/each}
						</div>
					{/if}
				</section>

				<section class="comparison" aria-labelledby="comparison-heading">
					<div class="section-heading comparison-heading">
						<div>
							<h3 id="comparison-heading">Visual comparison</h3>
							<p>Normalized browser render against the verified reference PDF.</p>
						</div>
						{#if selectedResult.comparisons.length > 0}
							<div class="flex items-center gap-2 text-muted max-[720px]:mt-3">
								<span class="mono-label">Page</span>
								<CompactSelectMenu
									label="Choose comparison page"
									value={String(selectedPageIndex + 1)}
									items={comparisonPageMenu()}
									onAction={(page) => (selectedPageIndex = page)}
									align="end"
									class="min-w-12"
								/>
							</div>
						{/if}
					</div>

					{#if !selectedPage}
						<p class="empty">Reference and generated pages appear here after the run.</p>
					{:else}
						<div class="view-controls" aria-label="Comparison mode">
							{#each ['side-by-side', 'opacity', 'swipe', 'heatmap'] as mode}
								<button
									type="button"
									class:active={viewMode === mode}
									onclick={() => (viewMode = mode as ViewMode)}
								>{mode.replace('-', ' ')}</button>
							{/each}
							<a
								href={selectedPage.actualSvgUrl}
								download={`sdocx-page-${selectedPage.pageIndex + 1}.svg`}>Download SVG</a
							>
						</div>

						{#if viewMode === 'opacity'}
							<label class="range-control">
								Generated opacity <strong>{opacity}%</strong>
								<input type="range" min="0" max="100" bind:value={opacity} />
							</label>
						{:else if viewMode === 'swipe'}
							<label class="range-control">
								Reveal generated <strong>{swipe}%</strong>
								<input type="range" min="0" max="100" bind:value={swipe} />
							</label>
						{/if}

						{#if viewMode === 'side-by-side'}
							<div class="side-by-side">
								<figure>
									<img src={selectedPage.actualRasterUrl} alt="Generated SDOCX page" />
									<figcaption>Generated SVG</figcaption>
								</figure>
								<figure>
									<img src={selectedPage.referenceUrl} alt="Samsung Notes reference PDF page" />
									<figcaption>Reference PDF</figcaption>
								</figure>
							</div>
						{:else if viewMode === 'heatmap'}
							<figure class="single-page heatmap">
								<img src={selectedPage.heatmapUrl} alt="Pixel difference heatmap" />
								<figcaption>Brighter red pixels have larger RGB differences.</figcaption>
							</figure>
						{:else}
							<div class="image-stack">
								<img src={selectedPage.referenceUrl} alt="Samsung Notes reference PDF page" />
								<img
									class="generated-layer"
									src={selectedPage.actualRasterUrl}
									alt="Generated SDOCX page"
									style={viewMode === 'opacity'
										? `opacity: ${opacity / 100}`
										: `clip-path: inset(0 ${100 - swipe}% 0 0)`}
								/>
								{#if viewMode === 'swipe'}
									<span class="swipe-line" style={`left: ${swipe}%`}></span>
								{/if}
							</div>
						{/if}

						<div class="metrics">
							<div><span>Mean absolute error</span><strong>{(selectedPage.metrics.meanAbsoluteError * 100).toFixed(2)}%</strong></div>
							<div><span>Root mean square</span><strong>{(selectedPage.metrics.rootMeanSquareError * 100).toFixed(2)}%</strong></div>
							<div><span>Changed pixels</span><strong>{(selectedPage.metrics.changedPixelRatio * 100).toFixed(2)}%</strong></div>
							<div><span>Raster size</span><strong>{selectedPage.metrics.width} × {selectedPage.metrics.height}</strong></div>
							<div><span>Generated source</span><strong>{selectedPage.metrics.generatedSourceWidth} × {selectedPage.metrics.generatedSourceHeight}</strong></div>
							<div><span>Reference source</span><strong>{selectedPage.metrics.referenceSourceWidth.toFixed(1)} × {selectedPage.metrics.referenceSourceHeight.toFixed(1)}</strong></div>
							<div class:mismatch={selectedPage.metrics.aspectRatioMismatch}>
								<span>Aspect ratio delta</span>
								<strong>{(selectedPage.metrics.aspectRatioDelta * 100).toFixed(2)}% {selectedPage.metrics.aspectRatioMismatch ? 'mismatch' : 'match'}</strong>
							</div>
						</div>
						<p class="metric-note">
							Visual metrics are informational. Source sizes use SVG units and PDF points; their aspect
							ratios are comparable. Fonts, rasterization, and antialiasing can change pixels without a
							parser regression.
						</p>
					{/if}
				</section>
			{/if}
		</section>
	</div>
</section>

<style>
	.regression-page { width: 100%; min-width: 0; padding: clamp(1.25rem, 3vw, 2.5rem); }
	.page-heading { display: flex; align-items: flex-end; justify-content: space-between; gap: 2rem; border-bottom: 1px solid var(--site-border); padding-bottom: 1.5rem; }
	.heading-copy { max-width: 48rem; }
	h1, h2, h3, p { margin-top: 0; }
	h1 { margin-bottom: 0.65rem; font-size: clamp(2rem, 4vw, 3.25rem); font-weight: 600; letter-spacing: -0.045em; line-height: 1; }
	.heading-copy p { max-width: 43rem; margin-bottom: 0; color: var(--site-muted); font-size: 1rem; line-height: 1.6; }
	.actions { display: flex; max-width: 34rem; flex-wrap: wrap; justify-content: flex-end; gap: 0.5rem; }
	.error-banner, .fixture-error { border: 1px solid color-mix(in srgb, #e45a4f 55%, var(--site-border)); border-radius: 0.45rem; background: color-mix(in srgb, #e45a4f 9%, var(--site-raised)); color: var(--site-text); font-size: 0.82rem; }
	.error-banner { margin-top: 1rem; padding: 0.8rem 0.9rem; }
	.workspace { display: grid; grid-template-columns: minmax(15rem, 20rem) minmax(0, 1fr); gap: 1rem; margin-top: 1rem; }
	.fixtures, .detail { min-width: 0; border: 1px solid var(--site-border); border-radius: 0.55rem; background: color-mix(in srgb, var(--site-raised) 92%, transparent); overflow: hidden; }
	.fixtures { align-self: start; }
	.panel-heading { display: flex; align-items: center; justify-content: space-between; gap: 1rem; border-bottom: 1px solid var(--site-border); padding: 1.15rem 1.25rem; }
	.panel-heading h2, .section-heading h3 { margin-bottom: 0; font-weight: 610; letter-spacing: -0.025em; }
	.panel-heading > span { color: var(--site-muted); font-size: 0.75rem; }
	.fixture { display: block; width: 100%; border: 0; border-bottom: 1px solid var(--site-border); background: transparent; padding: 1rem 1.15rem; color: var(--site-text); text-align: left; cursor: pointer; }
	.fixture:last-child { border-bottom: 0; }
	.fixture:hover, .fixture.chosen { background: color-mix(in srgb, #167bff 7%, var(--site-raised)); }
	.fixture.chosen { box-shadow: inset 2px 0 #167bff; }
	.fixture-top { display: flex; align-items: center; justify-content: space-between; gap: 0.75rem; }
	.fixture-top strong { font-size: 0.86rem; }
	.status { border-radius: 999px; background: var(--site-surface); padding: 0.25rem 0.4rem; color: var(--site-muted); font-size: 0.57rem; letter-spacing: 0.04em; }
	.status[data-tone='active'] { background: color-mix(in srgb, #167bff 12%, var(--site-surface)); color: #167bff; }
	.status[data-tone='success'] { background: color-mix(in srgb, #4e9c70 15%, var(--site-surface)); color: #4e9c70; }
	.status[data-tone='danger'] { background: color-mix(in srgb, #e45a4f 14%, var(--site-surface)); color: #e45a4f; }
	.message, .expectation { display: block; margin-top: 0.4rem; color: var(--site-muted); font-size: 0.74rem; line-height: 1.4; }
	.expectation { font-family: var(--font-mono); font-size: 0.62rem; }
	.duration { color: var(--site-muted); }
	.fixture-error { margin: 1.1rem 1.25rem 0; padding: 0.75rem 0.85rem; }
	.local-source, .checks, .comparison { padding: 1.35rem 1.25rem; }
	.checks, .comparison { border-top: 1px solid var(--site-border); }
	.local-source { background: color-mix(in srgb, var(--site-surface) 55%, transparent); }
	.section-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: 1.25rem; margin-bottom: 1rem; }
	.section-heading p { margin: 0.3rem 0 0; color: var(--site-muted); font-size: 0.78rem; }
	.section-heading > strong { color: #4e9c70; font-family: var(--font-mono); font-size: 1rem; }
	.section-heading > strong.failed { color: #e45a4f; }
	.text-button { border: 0; background: transparent; padding: 0; color: var(--site-muted); font-family: var(--font-mono); font-size: 0.66rem; cursor: pointer; }
	.text-button:hover { color: #167bff; }
	.local-files { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0.65rem; }
	.file-control { display: grid; min-width: 0; gap: 0.25rem; border: 1px dashed var(--site-border); border-radius: 0.45rem; background: var(--site-raised); padding: 0.85rem; cursor: pointer; }
	.file-control:hover { border-color: color-mix(in srgb, #167bff 65%, var(--site-border)); }
	.file-control input { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); clip-path: inset(50%); white-space: nowrap; }
	.file-control span { color: #167bff; }
	.file-control strong { overflow: hidden; font-size: 0.78rem; text-overflow: ellipsis; white-space: nowrap; }
	.file-control small, .hash-note, .metric-note { color: var(--site-muted); font-size: 0.68rem; }
	.hash-note { margin: 0.7rem 0 0; font-family: var(--font-mono); line-height: 1.5; }
	.empty { margin-bottom: 0; border: 1px dashed var(--site-border); border-radius: 0.45rem; padding: 2rem; color: var(--site-muted); font-size: 0.8rem; text-align: center; }
	.check-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0.5rem; }
	.check-row { display: flex; min-width: 0; align-items: flex-start; gap: 0.6rem; border: 1px solid color-mix(in srgb, #4e9c70 30%, var(--site-border)); border-radius: 0.4rem; background: color-mix(in srgb, #4e9c70 6%, var(--site-raised)); padding: 0.65rem 0.7rem; }
	.check-row.check-failed { border-color: color-mix(in srgb, #e45a4f 40%, var(--site-border)); background: color-mix(in srgb, #e45a4f 6%, var(--site-raised)); }
	.check-icon { display: grid; width: 1.15rem; height: 1.15rem; flex: 0 0 1.15rem; place-items: center; border-radius: 50%; background: #4e9c70; color: white; font-size: 0.72rem; font-weight: 700; }
	.check-failed .check-icon { background: #e45a4f; }
	.check-row strong, .check-row small { display: block; overflow: hidden; text-overflow: ellipsis; }
	.check-row strong { font-size: 0.75rem; }
	.check-row small { margin-top: 0.18rem; color: var(--site-muted); font-family: var(--font-mono); font-size: 0.6rem; white-space: nowrap; }
	.view-controls { display: flex; flex-wrap: wrap; gap: 0.4rem; margin-bottom: 0.8rem; }
	.view-controls button, .view-controls a { border: 1px solid var(--site-border); border-radius: 0.35rem; background: var(--site-surface); padding: 0.42rem 0.6rem; color: var(--site-muted); font-family: var(--font-mono); font-size: 0.63rem; text-decoration: none; text-transform: lowercase; cursor: pointer; }
	.view-controls button:hover, .view-controls button.active, .view-controls a:hover { border-color: color-mix(in srgb, #167bff 65%, var(--site-border)); color: #167bff; }
	.view-controls a { margin-left: auto; }
	.range-control { display: grid; grid-template-columns: auto auto 1fr; align-items: center; gap: 0.6rem; margin: 0.65rem 0 0.9rem; color: var(--site-muted); font-family: var(--font-mono); font-size: 0.65rem; }
	.range-control input { width: 100%; accent-color: #167bff; }
	.side-by-side { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0.65rem; }
	figure { min-width: 0; margin: 0; }
	figure img, .image-stack img { display: block; width: 100%; height: auto; border: 1px solid var(--site-border); background: white; }
	figcaption { padding: 0.45rem 0.1rem; color: var(--site-muted); font-family: var(--font-mono); font-size: 0.62rem; }
	.single-page, .image-stack { max-width: 52rem; margin-inline: auto; }
	.heatmap img { background: #111; }
	.image-stack { position: relative; overflow: hidden; }
	.image-stack .generated-layer { position: absolute; inset: 0; }
	.swipe-line { position: absolute; top: 0; bottom: 0; width: 2px; background: #167bff; box-shadow: 0 0 0 1px white; transform: translateX(-1px); }
	.metrics { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 0.45rem; margin-top: 0.8rem; }
	.metrics div { border: 1px solid var(--site-border); border-radius: 0.35rem; background: var(--site-surface); padding: 0.65rem; }
	.metrics span, .metrics strong { display: block; }
	.metrics span { color: var(--site-muted); font-family: var(--font-mono); font-size: 0.58rem; }
	.metrics strong { margin-top: 0.3rem; font-size: 0.82rem; font-variant-numeric: tabular-nums; }
	.metrics .mismatch strong { color: #e45a4f; }
	.metric-note { margin: 0.7rem 0 0; line-height: 1.5; }
	@media (max-width: 980px) {
		.page-heading { display: block; }
		.actions { justify-content: flex-start; margin-top: 1.25rem; }
		.workspace { grid-template-columns: 1fr; }
		.check-grid { grid-template-columns: 1fr; }
	}
	@media (max-width: 650px) {
		.local-files, .side-by-side, .metrics { grid-template-columns: 1fr; }
		.comparison-heading { display: block; }
	}
</style>
