<script lang="ts">
	import type { FixtureRunResult } from '$lib/regression/runner';
	import { statusTone } from '$lib/regression/view-model';

	interface Props {
		results: FixtureRunResult[];
		selectedId: string;
		onSelect: (id: string) => void;
	}

	let { results, selectedId, onSelect }: Props = $props();
</script>

<aside
	class="fixtures min-w-0 self-start overflow-hidden rounded-[0.55rem] border border-subtle bg-raised/90"
	aria-label="Compatibility fixtures"
>
	<div class="flex items-center justify-between gap-4 border-b border-subtle px-5 py-4.5">
		<h2 class="m-0 font-[610] tracking-[-0.025em]">Fixtures</h2>
		<span class="text-xs text-muted">{results.length}</span>
	</div>

	{#each results as result}
		<button
			type="button"
			class="fixture block w-full cursor-pointer border-0 border-b border-subtle bg-transparent px-4.5 py-4 text-left text-text transition-[background-color,box-shadow,transform] duration-[var(--motion-fast)] ease-[var(--ease-standard)] last:border-b-0 hover:bg-accent/[0.07] {result
				.fixture.id === selectedId
				? 'chosen bg-accent/[0.07] [box-shadow:inset_2px_0_#167bff]'
				: ''}"
			onclick={() => onSelect(result.fixture.id)}
		>
			<span class="flex items-center justify-between gap-3">
				<strong class="text-[0.86rem]">{result.fixture.id}</strong>
				<span
					class="mono-label rounded-full px-1.5 py-1 text-[0.57rem] tracking-[0.04em] transition-colors duration-[var(--motion-standard)] ease-[var(--ease-standard)] {statusTone(
						result.status
					) === 'active'
						? 'bg-accent/10 text-accent'
						: statusTone(result.status) === 'success'
							? 'bg-positive/15 text-positive'
							: statusTone(result.status) === 'danger'
								? 'bg-negative/15 text-negative'
								: 'bg-surface text-muted'}"
					data-tone={statusTone(result.status)}>{result.status}</span
				>
			</span>
			<span class="mt-1.5 block text-[0.74rem] leading-5 text-muted">{result.message}</span>
			<span class="mt-1.5 block font-mono text-[0.62rem] leading-5 text-muted">
				{result.fixture.storedPages} stored · {result.fixture.visiblePages} visible pages
			</span>
		</button>
	{/each}
</aside>
