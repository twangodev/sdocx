<script lang="ts">
	import type { LibraryWorkspace } from '$lib/library/workspace.svelte';
	let { library, onTemporary }: { library: LibraryWorkspace; onTemporary: (file: File) => void } =
		$props();
	const counts = $derived({
		imported: library.results.filter((result) => result.status === 'imported').length,
		duplicates: library.results.filter((result) => result.status === 'duplicate').length,
		failed: library.results.filter((result) => result.status === 'failed').length,
		skipped: library.results.filter((result) => result.status === 'skipped').length
	});
</script>

{#if library.importing || library.results.length || library.cancelled}
	<div class="import-status border-b border-subtle bg-surface px-4 py-3 text-xs">
		{#if library.importing}
			<div role="status">
				Importing {library.progress?.filename} · {library.progress?.completed ?? 0} / {library
					.progress?.total ?? 0}
			</div>
			<button class="mt-2 underline" onclick={() => library.cancelImport()}>Cancel import</button>
		{:else}
			<div class="flex items-center justify-between gap-3">
				<p role="status">
					{library.cancelled ? 'Import cancelled · ' : ''}{counts.imported} imported · {counts.duplicates}
					duplicates · {counts.failed} failed{counts.skipped ? ` · ${counts.skipped} skipped` : ''}
				</p>
				<button
					aria-label="Dismiss import results"
					onclick={() => {
						library.results = [];
						library.cancelled = false;
					}}>Dismiss</button
				>
			</div>
		{/if}
		{#each library.results as result, index (index)}
			{#if result.status === 'failed'}
				<p class="mt-2 text-muted">
					{result.filename}: {result.error}
					{#if result.unsavedFile}<button
							class="ml-2 text-text underline"
							onclick={() => result.unsavedFile && onTemporary(result.unsavedFile)}
							>Open temporarily</button
						>{/if}
				</p>
			{/if}
		{/each}
	</div>
{/if}
