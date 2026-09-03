<script lang="ts">
	import { Check, X } from '@lucide/svelte';
	import type { StructuralCheck } from '$lib/regression/structure';
	import SectionHeading from './SectionHeading.svelte';

	let { checks }: { checks: StructuralCheck[] } = $props();
	const passed = $derived(checks.filter((check) => check.passed).length);
	const failed = $derived(checks.some((check) => !check.passed));
</script>

<section class="border-t border-subtle px-5 py-5.5" aria-labelledby="checks-heading">
	<SectionHeading
		id="checks-heading"
		title="Structural checks"
		description="Exact assertions mirrored from the Rust external-corpus test."
	>
		{#snippet aside()}
			{#if checks.length > 0}
				<strong class="font-mono text-base {failed ? 'text-negative' : 'text-positive'}">
					{passed}/{checks.length}
				</strong>
			{/if}
		{/snippet}
	</SectionHeading>

	{#if checks.length === 0}
		<p
			class="m-0 rounded-[0.45rem] border border-dashed border-subtle p-8 text-center text-[0.8rem] text-muted"
		>
			Run this fixture to inspect its structure.
		</p>
	{:else}
		<div class="grid grid-cols-2 gap-2 max-[980px]:grid-cols-1">
			{#each checks as item}
				<div
					class="motion-fade-in flex min-w-0 items-start gap-2.5 rounded-[0.4rem] border p-2.5 {item.passed
						? 'border-positive/30 bg-positive/[0.06]'
						: 'border-negative/40 bg-negative/[0.06]'}"
				>
					<span
						class="grid size-[1.15rem] shrink-0 place-items-center rounded-full text-white {item.passed
							? 'bg-positive'
							: 'bg-negative'}"
						aria-hidden="true"
					>
						{#if item.passed}
							<Check size={11} strokeWidth={2.5} />
						{:else}
							<X size={11} strokeWidth={2.5} />
						{/if}
					</span>
					<div class="min-w-0">
						<strong class="block truncate text-[0.75rem]">{item.label}</strong>
						<small class="mt-[0.18rem] block truncate font-mono text-[0.6rem] text-muted">
							Expected {String(item.expected)} · got {String(item.actual)}
						</small>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</section>
