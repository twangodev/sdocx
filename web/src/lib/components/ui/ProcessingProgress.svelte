<script lang="ts">
	import type { Snippet } from 'svelte';
	import { LoaderCircle } from '@lucide/svelte';
	import { progressLabel, type ProcessingProgress } from '$converter/progress';
	import Notification from './Notification.svelte';

	let { progress, detail, floating = false, anchored = false, actions }: {
		progress: ProcessingProgress | null | undefined;
		detail?: string;
		floating?: boolean;
		anchored?: boolean;
		actions?: Snippet;
	} = $props();
	const fraction = $derived(progress?.total !== null && progress?.total !== undefined
		? progress.total === 0 ? 1 : Math.min(1, Math.max(0, progress.completed / progress.total)) : undefined);
</script>

{#if progress}
	<Notification label="Processing progress" {floating} {anchored} {actions}>
		{#snippet icon()}<LoaderCircle size={14} class="animate-spin text-muted" />{/snippet}
		<div class="flex flex-wrap items-center justify-between gap-x-3 gap-y-1 text-xs">
			<span role="status" class="font-medium">{progressLabel(progress)}</span>
			{#if fraction !== undefined}
				<span class="rounded bg-surface px-1.5 py-0.5 font-mono text-[10px] text-muted tabular-nums">{Math.floor(fraction * 100)}%</span>
			{/if}
		</div>
		{#if detail}<p class="mt-0.5 truncate text-left text-[11px] text-muted" title={detail}>{detail}</p>{/if}
		<div class="relative mt-2 h-1 overflow-hidden rounded-full bg-subtle">
			<progress aria-label={progressLabel(progress)} max="1" value={fraction} class="block h-full w-full"></progress>
			{#if fraction === undefined}<span aria-hidden="true" class="indeterminate absolute inset-y-0 left-0 w-1/3 rounded-full bg-accent"></span>{/if}
		</div>
		{#if fraction !== undefined}
			<p aria-hidden="true" class="mt-1 text-left font-mono text-[10px] text-muted tabular-nums">{progress.completed.toLocaleString()} / {progress.total?.toLocaleString()}</p>
		{/if}
	</Notification>
{/if}

<style>
	progress { appearance: none; border: 0; background: transparent; color: var(--color-accent); }
	progress::-webkit-progress-bar { background: transparent; }
	progress::-webkit-progress-value { background: var(--color-accent); border-radius: 999px; }
	progress::-moz-progress-bar { background: var(--color-accent); border-radius: 999px; }
	progress:indeterminate { opacity: 0; }
	.indeterminate { animation: progress-travel 1.4s ease-in-out infinite; }
	@keyframes progress-travel {
		from { transform: translateX(-100%); }
		to { transform: translateX(300%); }
	}
	@media (prefers-reduced-motion: reduce) {
		.indeterminate { animation: none; left: 33%; }
	}
</style>
