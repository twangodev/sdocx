<script lang="ts">
	import { onMount } from 'svelte';
	import { CircleCheck, TriangleAlert, X } from '@lucide/svelte';
	import IconButton from './ui/IconButton.svelte';
	import Notification from './ui/Notification.svelte';

	let { codes, failed = false, anchored = false, onDismiss }: {
		codes: string[];
		failed?: boolean;
		anchored?: boolean;
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

<Notification label="Document upload notification" floating {anchored} {leaving}>
	{#snippet icon()}
		<div class:text-muted={hasIssues} class:text-success={!hasIssues}>
			{#if hasIssues}<TriangleAlert size={14} />{:else}<CircleCheck size={14} />{/if}
		</div>
	{/snippet}
	{#snippet actions()}
		<IconButton label="Dismiss notification" onclick={onDismiss}><X size={12} /></IconButton>
	{/snippet}
	<p role="status" class="text-xs font-medium">{failed ? 'Import failed' : codes.length ? `Imported with ${codes.length} ${codes.length === 1 ? 'warning' : 'warnings'}` : 'Successfully imported'}</p>
	<div class="mt-0.5 flex flex-wrap items-center gap-x-1 text-[11px] text-muted">
		<a href={issueUrl} target="_blank" rel="noreferrer" class="hover:text-accent hover:underline">Report an issue</a>
		<span>or</span>
		<a href="https://github.com/twangodev/sdocx/issues/new?title=Feature%20request" target="_blank" rel="noreferrer" class="hover:text-accent hover:underline">request a feature</a>
	</div>
</Notification>
