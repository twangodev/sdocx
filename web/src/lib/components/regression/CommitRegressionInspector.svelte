<script lang="ts">
	import { Check, Minus, X } from '@lucide/svelte';
	import type { CommitFixtureResult } from '$lib/regression/commit-runner';
	import RegressionPanel from './RegressionPanel.svelte';

	interface Props {
		result: CommitFixtureResult;
		leftLabel: string;
		rightLabel: string;
	}

	let { result, leftLabel, rightLabel }: Props = $props();
	const structuralChanges = $derived(result.diff?.structuralDiffs.filter((item) => item.changed) ?? []);
	const leftPassed = $derived(result.diff?.leftChecks.filter((item) => item.passed).length ?? 0);
	const rightPassed = $derived(result.diff?.rightChecks.filter((item) => item.passed).length ?? 0);
	const checkCount = $derived(result.diff?.leftChecks.length ?? 0);

	function short(value: string | number | boolean | undefined): string {
		if (value === undefined) return '—';
		const output = String(value);
		return output.length > 28 ? `${output.slice(0, 27)}…` : output;
	}
</script>

<aside
	class="motion-surface-in w-64 shrink-0 overflow-y-auto border-l border-subtle bg-bg max-[980px]:hidden"
	aria-label="Comparison details"
>
	<div class="flex h-9 items-center border-b border-subtle px-3 text-[11px] tracking-[0.04em] text-muted">
		inspection
	</div>

	<RegressionPanel title="result" meta={result.status}>
		<div class="space-y-2 text-[11px]">
			<div class="flex items-center justify-between gap-3">
				<span class="text-muted">visual pages</span>
				<span class="font-mono text-text">
					{result.diff ? `${result.diff.changedPageCount} changed` : '—'}
				</span>
			</div>
			<div class="flex items-center justify-between gap-3">
				<span class="text-muted">structure</span>
				<span class="font-mono text-text">
					{result.diff ? `${structuralChanges.length} changed` : '—'}
				</span>
			</div>
			<div class="flex items-center justify-between gap-3">
				<span class="text-muted">runtime</span>
				<span class="font-mono text-text">{result.durationMs ? `${result.durationMs} ms` : '—'}</span>
			</div>
		</div>
	</RegressionPanel>

	<RegressionPanel title="checks" meta={checkCount ? `${leftPassed}/${rightPassed}` : undefined}>
		{#if !result.diff}
			<p class="m-0 text-[11px] leading-5 text-muted">Run the comparison to inspect structure.</p>
		{:else}
			<div class="mb-2 grid grid-cols-[1fr_auto_auto] gap-2 font-mono text-[9px] text-muted">
				<span></span><span>{leftLabel}</span><span>{rightLabel}</span>
			</div>
			<div class="space-y-1.5">
				{#each result.diff.structuralDiffs as item (item.id)}
					<div class="grid grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-2 text-[10px]">
						<span class="flex min-w-0 items-center gap-1.5 truncate text-muted" title={item.label}>
							{#if item.changed}
								<X size={10} class="shrink-0 text-negative" />
							{:else if item.left === true}
								<Check size={10} class="shrink-0 text-positive" />
							{:else}
								<Minus size={10} class="shrink-0 text-muted" />
							{/if}
							{item.label}
						</span>
						<span class="max-w-14 truncate font-mono text-text" title={String(item.left)}>{short(item.left)}</span>
						<span class="max-w-14 truncate font-mono text-text" title={String(item.right)}>{short(item.right)}</span>
					</div>
				{/each}
			</div>
		{/if}
	</RegressionPanel>
</aside>
