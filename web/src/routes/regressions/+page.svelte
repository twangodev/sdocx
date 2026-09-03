<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import Logo from '$lib/components/Logo.svelte';
	import CommitRefForm from '$lib/components/regression/CommitRefForm.svelte';
	import { loadRendererCatalog } from '$lib/regression/renderer-catalog';

	let leftRef = $state('current');
	let rightRef = $state('current');
	let available = $state(1);

	onMount(async () => {
		const catalog = await loadRendererCatalog();
		const prepared = catalog.renderers.filter((renderer) => renderer.sha !== 'current');
		available = catalog.renderers.length;
		if (prepared.length >= 2) {
			leftRef = prepared[0].sha.slice(0, 12);
			rightRef = prepared[1].sha.slice(0, 12);
		} else if (prepared.length === 1) {
			leftRef = prepared[0].sha.slice(0, 12);
		}
	});

	function compare(left: string, right: string): void {
		void goto(`/regressions/${encodeURIComponent(left)}/vs/${encodeURIComponent(right)}`);
	}
</script>

<svelte:head>
	<title>sdocx — commit regressions</title>
	<meta
		name="description"
		content="Compare SDOCX rendering and document structure between two prebuilt Git commits."
	/>
</svelte:head>

<section class="flex min-h-[calc(100svh-2.5rem)] w-full items-center justify-center bg-bg px-6 py-12">
	<div class="motion-surface-in w-full max-w-lg">
		<div class="flex items-center gap-2">
			<Logo size={22} />
			<h1 class="m-0 text-[14px] font-medium tracking-tight">compare renderers</h1>
		</div>
		<p class="mt-3 mb-0 text-[13px] leading-5 text-muted">
			Run the same compatibility corpus through two commits. Rendering stays in this browser.
		</p>

		<div class="mt-7">
			<CommitRefForm {leftRef} {rightRef} onCompare={compare} />
		</div>

		<div class="mt-5 flex items-start justify-between gap-6 border-t border-subtle pt-3 font-mono text-[10px] leading-4 text-muted">
			<span>{available} renderer{available === 1 ? '' : 's'} available</span>
			<code class="text-right">bun run regression:prepare -- &lt;from&gt; &lt;to&gt;</code>
		</div>
	</div>
</section>
