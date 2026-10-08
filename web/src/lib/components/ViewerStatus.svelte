<script lang="ts">
	import type { WorkerPhase } from '$converter/protocol';
	import type { ProcessingProgress as WorkProgress } from '$converter/progress';
	import ProcessingProgress from './ui/ProcessingProgress.svelte';

	interface Props {
		phase: WorkerPhase | null;
		status: string;
		exporting: boolean;
		exportProgress: string;
		progress?: WorkProgress | null;
	}

	let { phase, status, exporting, exportProgress, progress }: Props = $props();
</script>

<div
	class="min-h-7 border-t border-subtle px-2.5 py-2 font-mono text-[0.59rem] text-muted"
	role="status"
	aria-live="polite"
>
	<span class="mr-1.5" class:text-positive={phase === 'ready'}>{phase === 'ready' ? '●' : '○'}</span>
	{exporting ? exportProgress : status}
	<ProcessingProgress {progress} />
</div>
