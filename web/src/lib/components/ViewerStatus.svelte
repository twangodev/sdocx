<script lang="ts">
	import type { WorkerPhase } from '$converter/protocol';

	interface Props {
		phase: WorkerPhase | null;
		status: string;
		exporting: boolean;
		exportProgress: string;
	}

	let { phase, status, exporting, exportProgress }: Props = $props();
</script>

<div class="status-line" role="status" aria-live="polite">
	<span class:ready={phase === 'ready'}>{phase === 'ready' ? '●' : '○'}</span>
	{exporting ? exportProgress : status}
</div>

<style>
	.status-line {
		min-height: 1.75rem;
		border-top: 1px solid var(--site-border);
		padding: 0.45rem 0.6rem;
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.59rem;
	}

	.status-line span {
		margin-right: 0.35rem;
	}

	.status-line span.ready {
		color: var(--color-success);
	}
</style>
