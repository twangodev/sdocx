<script lang="ts">
	import { onMount } from 'svelte';
	import { CircleCheck, TriangleAlert, X } from '@lucide/svelte';
	import IconButton from './ui/IconButton.svelte';

	let { codes, failed = false, onDismiss }: {
		codes: string[];
		failed?: boolean;
		onDismiss: () => void;
	} = $props();
	let leaving = $state(false);
	onMount(() => {
		let timer = window.setTimeout(() => {
			leaving = true;
			timer = window.setTimeout(onDismiss, 180);
		}, 6000);
		return () => window.clearTimeout(timer);
	});
	const hasIssues = $derived(failed || codes.length > 0);
	const issueUrl = $derived('https://github.com/twangodev/sdocx/issues/new?' + new URLSearchParams({
		title: failed ? 'Document could not be opened' : 'Document rendering issue',
		body: `## What went wrong?\n\nDescribe what you expected and what you saw.\n\n## Diagnostics\n\n${codes.length ? [...new Set(codes)].slice(0, 20).map(code => `- ${code.slice(0, 100)}`).join('\n') : failed ? 'Document failed to open. Add the error shown in the app.' : 'No parser warnings were reported.'}\n\n## Sample (optional)\n\nIf you can share a non-sensitive .sdocx file and a screenshot or PDF from Samsung Notes, attach them here.\n\nSamsung Notes version / device:\nBrowser:`
	}));
</script>

<div class="pointer-events-none fixed top-12 left-1/2 z-50 w-max max-w-[calc(100%-2rem)] -translate-x-1/2">
	<aside aria-label="Document upload notification" class="toast pointer-events-auto flex items-start gap-2 rounded-lg border border-subtle bg-raised px-3 py-2 text-text shadow-lg" class:leaving>
		<div class="mt-0.5 shrink-0" class:text-muted={hasIssues} class:text-success={!hasIssues}>
			{#if hasIssues}<TriangleAlert size={14} />{:else}<CircleCheck size={14} />{/if}
		</div>
		<div class="min-w-0">
			<p role="status" class="text-xs font-medium">{failed ? 'Import failed' : codes.length ? `Imported with ${codes.length} ${codes.length === 1 ? 'warning' : 'warnings'}` : 'Successfully imported'}</p>
			<div class="mt-0.5 flex flex-wrap items-center gap-x-1 text-[11px] text-muted">
				<a href={issueUrl} target="_blank" rel="noreferrer" class="hover:text-accent hover:underline">Report an issue</a>
				<span>or</span>
				<a href="https://github.com/twangodev/sdocx/issues/new?title=Feature%20request" target="_blank" rel="noreferrer" class="hover:text-accent hover:underline">request a feature</a>
			</div>
		</div>
		<IconButton label="Dismiss notification" onclick={onDismiss}><X size={12} /></IconButton>
	</aside>
</div>

<style>
	.toast { animation: toast-in 180ms var(--ease-out) both; }
	.toast.leaving { animation: toast-out 180ms var(--ease-standard) both; }
	@keyframes toast-in {
		from { opacity: 0; transform: translateY(-12px); }
		to { opacity: 1; transform: translateY(0); }
	}
	@keyframes toast-out {
		from { opacity: 1; transform: translateY(0); }
		to { opacity: 0; transform: translateY(-12px); }
	}
</style>
