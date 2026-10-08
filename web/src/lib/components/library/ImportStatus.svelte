<script lang="ts">
	import { X } from '@lucide/svelte';
	import Button from '../ui/Button.svelte';
	import ProcessingProgress from '../ui/ProcessingProgress.svelte';
	import IconButton from '../ui/IconButton.svelte';
	import type { LibraryWorkspace } from '$lib/library/workspace.svelte';
	import { pendingProgress } from '$converter/progress';
	let { library, onTemporary }: { library: LibraryWorkspace; onTemporary: (file: File) => void } =
		$props();
	const counts = $derived({
		imported: library.results.filter((result) => result.status === 'imported').length,
		duplicates: library.results.filter((result) => result.status === 'duplicate').length,
		failed: library.results.filter((result) => result.status === 'failed').length,
		skipped: library.results.filter((result) => result.status === 'skipped').length
	});
</script>

{#if library.importing}
	<ProcessingProgress
		progress={library.progress?.work ?? pendingProgress('reading')}
		detail={`${library.progress?.completed ?? 0} / ${library.progress?.total ?? 0} files · ${library.progress?.filename ?? 'Importing notes'}`}
		floating
	>
		{#snippet actions()}<Button size={7} tone="ghost" aria-label="Cancel import" onclick={() => library.cancelImport()}>Cancel</Button>{/snippet}
	</ProcessingProgress>
{/if}

{#if library.importing ? counts.failed : library.results.length || library.cancelled}
	<div class="import-status shrink-0 border-b border-subtle bg-bg px-3 text-[11px] text-muted">
		{#if !library.importing}
			<div class="flex min-h-9 items-center justify-between gap-3">
				<p role="status">
					{library.cancelled ? 'Import cancelled · ' : ''}{counts.imported} imported{counts.duplicates
						? ` · ${counts.duplicates} duplicates`
						: ''}{counts.failed ? ` · ${counts.failed} failed` : ''}{counts.skipped
						? ` · ${counts.skipped} skipped`
						: ''}
				</p>
				<IconButton
					label="Dismiss import results"
					size={7}
					onclick={() => {
						library.results = [];
						library.cancelled = false;
					}}><X size={13} /></IconButton
				>
			</div>
		{/if}
		{#if counts.failed}
			<div class="max-h-36 overflow-y-auto pb-2">
				{#each library.results as result, index (index)}
					{#if result.status === 'failed'}
						<div class="flex flex-wrap items-center gap-x-2 py-1">
							<p class="min-w-0 break-words">{result.filename}: {result.error}</p>
							{#if result.unsavedFile}<Button
									size={7}
									tone="ghost"
									onclick={() => result.unsavedFile && onTemporary(result.unsavedFile)}
									>Open temporarily</Button
								>{/if}
						</div>
					{/if}
				{/each}
			</div>
		{/if}
	</div>
{/if}
