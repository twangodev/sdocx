<script lang="ts">
	import { CircleAlert, CircleCheck, CircleDotDashed } from '@lucide/svelte';
	import type { CommitFixtureResult } from '$lib/regression/commit-runner';

	interface Props {
		results: CommitFixtureResult[];
		selectedId: string;
		onSelect: (id: string) => void;
	}

	let { results, selectedId, onSelect }: Props = $props();

	function tone(status: CommitFixtureResult['status']): string {
		if (status === 'changed' || status === 'failed') return 'text-negative';
		if (status === 'unchanged') return 'text-positive';
		if (status === 'downloading' || status === 'rendering') return 'text-accent';
		return 'text-muted/65';
	}
</script>

<aside
	class="motion-panel-left flex w-60 shrink-0 flex-col border-r border-subtle bg-bg max-[760px]:w-48"
	aria-label="Compatibility fixtures"
>
	<div class="flex h-9 shrink-0 items-center justify-between border-b border-subtle px-3">
		<span class="text-[11px] tracking-[0.04em] text-muted">corpus</span>
		<span class="font-mono text-[10px] text-muted">{results.length}</span>
	</div>
	<nav class="min-h-0 flex-1 space-y-0.5 overflow-y-auto p-2" aria-label="Fixtures">
		{#each results as result (result.fixture.id)}
			<button
				type="button"
				class="flex min-h-10 w-full cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-left transition-colors {selectedId ===
				result.fixture.id
					? 'bg-surface text-text'
					: 'text-muted hover:bg-surface/60 hover:text-text'}"
				onclick={() => onSelect(result.fixture.id)}
			>
				<span class="shrink-0 {tone(result.status)}" aria-hidden="true">
					{#if result.status === 'unchanged'}
						<CircleCheck size={13} strokeWidth={1.5} />
					{:else if result.status === 'changed' || result.status === 'failed'}
						<CircleAlert size={13} strokeWidth={1.5} />
					{:else}
						<CircleDotDashed
							size={13}
							strokeWidth={1.5}
							class={result.status === 'downloading' || result.status === 'rendering'
								? 'animate-spin'
								: ''}
						/>
					{/if}
				</span>
				<span class="min-w-0 flex-1">
					<strong class="block truncate text-[11px] font-medium">{result.fixture.id}</strong>
					<span class="mt-0.5 block truncate font-mono text-[10px] text-muted">
						{result.message}
					</span>
				</span>
			</button>
		{/each}
	</nav>
</aside>
