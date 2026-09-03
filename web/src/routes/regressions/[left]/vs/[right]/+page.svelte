<script lang="ts">
	import { page } from '$app/state';
	import CommitRegressionWorkspace from '$lib/components/regression/CommitRegressionWorkspace.svelte';

	const leftRef = $derived(page.params.left ?? 'current');
	const rightRef = $derived(page.params.right ?? 'current');
</script>

<svelte:head>
	<title>{leftRef.slice(0, 8)} vs {rightRef.slice(0, 8)} — sdocx regressions</title>
	<meta
		name="description"
		content={`Compare SDOCX renderer commits ${leftRef} and ${rightRef} in the browser.`}
	/>
</svelte:head>

{#key `${leftRef}:${rightRef}`}
	<CommitRegressionWorkspace {leftRef} {rightRef} />
{/key}
