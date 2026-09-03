<script lang="ts">
	import { onDestroy } from 'svelte';
	import FixtureDetail from '$lib/components/regression/FixtureDetail.svelte';
	import FixtureList from '$lib/components/regression/FixtureList.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import { CORPUS_FIXTURES } from '$lib/regression/manifest';
	import { disposeComparisonPages } from '$lib/regression/pdf';
	import {
		createRegressionReport,
		downloadRegressionArchive,
		downloadReport,
		type RegressionReport
	} from '$lib/regression/report';
	import {
		RegressionSuiteRunner,
		type FixtureRunResult,
		type LocalFixtureAssets,
		type SuiteExecution
	} from '$lib/regression/runner';

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
	let selectedResult: FixtureRunResult | undefined;
	let selectedLocal: LocalSelection | undefined;
	let localSelections: Record<string, LocalSelection> = {};
	let uiError = '';

	$: selectedResult = results.find((result) => result.fixture.id === selectedId) ?? results[0];
	$: selectedLocal = selectedResult ? localSelections[selectedResult.fixture.id] : undefined;

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
		try {
			execution = await runner.run(
				fixtures,
				(updated) => replaceResults(updated, merge),
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

	function selectFixture(id: string): void {
		selectedId = id;
		uiError = '';
	}

	function chooseLocalFile(kind: keyof LocalFixtureAssets, file: File): void {
		if (!selectedResult) return;
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

<section
	class="motion-surface-in w-full min-w-0 p-[clamp(1.25rem,3vw,2.5rem)]"
	aria-labelledby="regression-title"
>
	<header
		class="flex items-end justify-between gap-8 border-b border-subtle pb-6 max-[980px]:block"
	>
		<div class="max-w-3xl">
			<h1
				class="mt-0 mb-2.5 text-[clamp(2rem,4vw,3.25rem)] leading-none font-semibold tracking-[-0.045em]"
				id="regression-title">Regression lab</h1
			>
			<p class="mb-0 max-w-2xl text-base leading-6 text-muted">
				Run the compatibility corpus and compare generated pages with Samsung Notes reference PDFs.
			</p>
		</div>
		<div class="flex max-w-lg flex-wrap justify-end gap-2 max-[980px]:mt-5 max-[980px]:justify-start">
			<Button onclick={runSelected} disabled={running || !selectedResult}>
				Run selected
			</Button>
			<Button variant="primary" onclick={runAll} disabled={running}>
				{running ? 'Running…' : 'Run all'}
			</Button>
			{#if running}
				<Button onclick={() => runner.cancel()}>Cancel</Button>
			{/if}
			{#if report}
				<Button onclick={() => downloadReport(report!, 'json')}>JSON</Button>
				<Button onclick={() => downloadReport(report!, 'html')}>HTML</Button>
				<Button onclick={downloadZip} disabled={packaging}>
					{packaging ? 'Packing…' : 'ZIP + images'}
				</Button>
			{/if}
		</div>
	</header>

	{#if uiError}
		<div
			class="motion-surface-in mt-4 rounded-md border border-danger/55 bg-danger/10 px-3.5 py-3 text-[0.82rem] text-text"
			role="alert"><strong>Could not continue.</strong> {uiError}</div
		>
	{/if}

	<div class="workspace mt-4 grid grid-cols-[minmax(15rem,20rem)_minmax(0,1fr)] gap-4 max-[980px]:grid-cols-1">
		<FixtureList {results} selectedId={selectedResult?.fixture.id ?? ''} onSelect={selectFixture} />
		{#if selectedResult}
			<FixtureDetail
				result={selectedResult}
				selection={selectedLocal}
				onChoose={chooseLocalFile}
				onClear={clearLocalFiles}
			/>
		{/if}
	</div>
</section>
