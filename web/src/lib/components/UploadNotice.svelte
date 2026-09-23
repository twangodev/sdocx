<script lang="ts">
	import { CircleCheck, TriangleAlert, ExternalLink, X } from '@lucide/svelte';
	import IconButton from './ui/IconButton.svelte';

	let { codes, failed = false, onDismiss, onDetails }: {
		codes: string[];
		failed?: boolean;
		onDismiss: () => void;
		onDetails: () => void;
	} = $props();
	const hasIssues = $derived(failed || codes.length > 0);
	const issueUrl = $derived('https://github.com/twangodev/sdocx/issues/new?' + new URLSearchParams({
		title: failed ? 'Document could not be opened' : 'Document rendering issue',
		body: `## What went wrong?\n\nDescribe what you expected and what you saw.\n\n## Diagnostics\n\n${codes.length ? [...new Set(codes)].slice(0, 20).map(code => `- ${code.slice(0, 100)}`).join('\n') : failed ? 'Document failed to open. Add the error shown in the app.' : 'No parser warnings were reported.'}\n\n## Sample (optional)\n\nIf you can share a non-sensitive .sdocx file and a screenshot or PDF from Samsung Notes, attach them here.\n\nSamsung Notes version / device:\nBrowser:`
	}));
</script>

<aside aria-label="Document upload notification" class="motion-surface-in fixed right-4 bottom-4 z-50 w-[min(23rem,calc(100%-2rem))] rounded-xl border border-subtle bg-raised p-3.5 text-text shadow-lg">
	<div class="flex items-start gap-2.5">
		<div class="mt-0.5 shrink-0" class:text-muted={hasIssues} class:text-success={!hasIssues}>
			{#if hasIssues}<TriangleAlert size={17} />{:else}<CircleCheck size={17} />{/if}
		</div>
		<div class="min-w-0 flex-1">
			<p role="status" class="text-xs font-semibold">{failed ? 'Could not open document' : codes.length ? `Opened with ${codes.length} parser ${codes.length === 1 ? 'warning' : 'warnings'}` : 'Document opened · no parser warnings'}</p>
			<p class="mt-1 text-xs leading-relaxed text-muted">{hasIssues ? 'Some features may not be supported yet.' : 'Something look off?'} This open-source project is reverse-engineering Samsung Notes. An issue helps us improve support.</p>
			<div class="mt-2.5 flex flex-wrap items-center gap-3 text-xs">
				<a href={issueUrl} target="_blank" rel="noreferrer" class="inline-flex items-center gap-1 text-accent hover:underline">Report an issue <ExternalLink size={12} /></a>
				{#if codes.length}<button class="text-muted hover:text-text" onclick={onDetails}>View warnings</button>{/if}
			</div>
			<p class="mt-2 text-[10px] text-muted">Your document stays local. Nothing is attached automatically.</p>
		</div>
		<IconButton label="Dismiss notification" onclick={onDismiss}><X size={13} /></IconButton>
	</div>
</aside>
