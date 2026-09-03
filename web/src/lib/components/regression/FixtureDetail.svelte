<script lang="ts">
	import type { FixtureRunResult, LocalFixtureAssets } from '$lib/regression/runner';
	import ComparisonViewer from './ComparisonViewer.svelte';
	import FixtureSource from './FixtureSource.svelte';
	import StructuralChecks from './StructuralChecks.svelte';

	interface Props {
		result: FixtureRunResult;
		selection?: Partial<LocalFixtureAssets>;
		onChoose: (kind: keyof LocalFixtureAssets, file: File) => void;
		onClear: () => void;
	}

	let { result, selection, onChoose, onClear }: Props = $props();
</script>

<section class="min-w-0 overflow-hidden rounded-[0.55rem] border border-subtle bg-raised/90">
	<div class="motion-fade-in flex items-center justify-between gap-4 border-b border-subtle px-5 py-4.5">
		<h2 class="m-0 font-[610] tracking-[-0.025em]">{result.fixture.id}</h2>
		{#if result.durationMs !== undefined}
			<span class="mono-label text-muted">{(result.durationMs / 1000).toFixed(1)} s</span>
		{/if}
	</div>

	{#if result.error}
		<div
			class="motion-surface-in mx-5 mt-4 rounded-[0.45rem] border border-negative/55 bg-negative/[0.09] px-3.5 py-3 text-[0.82rem] text-text"
		>
			<strong>Run did not pass.</strong> {result.error}
		</div>
	{/if}

	<FixtureSource fixture={result.fixture} {selection} {onChoose} {onClear} />
	<StructuralChecks checks={result.checks} />
	<ComparisonViewer fixtureId={result.fixture.id} comparisons={result.comparisons} />
</section>
