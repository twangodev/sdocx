<script lang="ts">
	import { progressLabel, type ProcessingProgress } from '$converter/progress';
	let { progress }: { progress: ProcessingProgress | null | undefined } = $props();
	const fraction = $derived(progress?.total !== null && progress?.total !== undefined
		? progress.total === 0 ? 1 : progress.completed / progress.total : undefined);
</script>

{#if progress}
	<div class="grid min-w-0 gap-1.5 py-1 text-xs text-muted">
		<div class="flex items-center justify-between gap-3">
			<span>{progressLabel(progress)}</span>
			{#if fraction !== undefined}
				<span class="shrink-0 font-mono tabular-nums">{progress.completed.toLocaleString()} / {progress.total?.toLocaleString()} · {Math.floor(fraction * 100)}%</span>
			{/if}
		</div>
		<progress aria-label={progressLabel(progress)} max="1" value={fraction} class="h-1.5 w-full overflow-hidden rounded-full"></progress>
	</div>
{/if}

<style>
	progress { accent-color: var(--color-text); }
	progress::-webkit-progress-bar { background: var(--color-subtle); border-radius: 999px; }
	progress::-webkit-progress-value { background: var(--color-text); border-radius: 999px; }
	progress::-moz-progress-bar { background: var(--color-text); border-radius: 999px; }
</style>
