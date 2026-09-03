<script lang="ts">
	import { goto } from '$app/navigation';
	import { ChevronLeft, Square } from '@lucide/svelte';
	import { onMount, untrack } from 'svelte';
	import IconButton from '$lib/components/ui/IconButton.svelte';
	import { CommitRegressionSession } from '$lib/regression/commit-session.svelte';
	import CommitFixtureRail from './CommitFixtureRail.svelte';
	import CommitPageCompare from './CommitPageCompare.svelte';
	import CommitRefForm from './CommitRefForm.svelte';
	import CommitRegressionInspector from './CommitRegressionInspector.svelte';

	let { leftRef, rightRef }: { leftRef: string; rightRef: string } = $props();
	const session = new CommitRegressionSession(
		untrack(() => leftRef),
		untrack(() => rightRef)
	);
	let selectedId = $state(session.results[0]?.fixture.id ?? '');
	const selectedResult = $derived(
		session.results.find((result) => result.fixture.id === selectedId) ?? session.results[0]
	);
	const leftLabel = $derived(shortRef(session.left?.sha ?? leftRef));
	const rightLabel = $derived(shortRef(session.right?.sha ?? rightRef));

	function shortRef(ref: string): string {
		return /^[a-f0-9]{8,40}$/i.test(ref) ? ref.slice(0, 8) : ref;
	}

	function compare(left: string, right: string): void {
		if (left === leftRef && right === rightRef) {
			void session.run();
			return;
		}
		void goto(`/regressions/${encodeURIComponent(left)}/vs/${encodeURIComponent(right)}`);
	}

	onMount(() => {
		void session.prepare();
		return () => session.destroy();
	});
</script>

<section
	class="flex h-[calc(100svh-2.5rem)] min-h-[32rem] w-full min-w-0 flex-col overflow-hidden"
	aria-label="Commit regression comparison"
>
	<header class="grid h-12 shrink-0 grid-cols-[1fr_minmax(24rem,38rem)_1fr] items-center gap-3 border-b border-subtle bg-bg px-2.5 max-[840px]:grid-cols-[1fr_auto]">
		<a
			href="/regressions"
			class="flex w-fit items-center gap-1 rounded px-1.5 py-1 text-[11px] text-muted no-underline transition-colors hover:bg-surface hover:text-text"
		>
			<ChevronLeft size={13} strokeWidth={1.5} />
			regressions
		</a>

		<div class="max-[840px]:hidden">
			<CommitRefForm {leftRef} {rightRef} compact disabled={session.running} onCompare={compare} />
		</div>

		<div class="flex min-w-0 items-center justify-end gap-2">
			{#if session.summary}
				<span class="truncate font-mono text-[10px] text-muted">
					{session.summary.changed} changed · {session.summary.failed} failed
				</span>
			{/if}
			{#if session.running}
				<IconButton label="Cancel comparison" tooltip size={7} onclick={() => session.cancel()}>
					<Square size={11} fill="currentColor" strokeWidth={1.5} />
				</IconButton>
			{/if}
		</div>
	</header>

	{#if session.preparing}
		<div class="grid min-h-0 flex-1 place-items-center bg-canvas">
			<div class="text-center">
				<span class="mx-auto mb-3 block size-4 animate-spin rounded-full border border-subtle border-t-accent" aria-hidden="true"></span>
				<p class="m-0 font-mono text-[11px] text-muted">resolving renderers</p>
			</div>
		</div>
	{:else if session.error && !session.ready}
		<div class="grid min-h-0 flex-1 place-items-center bg-canvas px-6">
			<div class="max-w-lg text-center">
				<p class="m-0 text-[12px] text-negative">{session.error}</p>
				<a href="/regressions" class="mt-4 inline-block text-[11px] text-muted hover:text-text">choose another pair</a>
			</div>
		</div>
	{:else if selectedResult}
		<div class="flex min-h-0 flex-1">
			<CommitFixtureRail
				results={session.results}
				{selectedId}
				onSelect={(id) => (selectedId = id)}
			/>
			<CommitPageCompare
				result={selectedResult}
				{leftLabel}
				{rightLabel}
				onRun={() => void session.run()}
			/>
			<CommitRegressionInspector result={selectedResult} {leftLabel} {rightLabel} />
		</div>
	{/if}
</section>
