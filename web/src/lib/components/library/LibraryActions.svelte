<script lang="ts">
	import { downloadBlob } from '$converter/files';
	import type { LibraryWorkspace } from '$lib/library/workspace.svelte';
	import type { Collection } from '$lib/library/model';
	import LibraryDialog from './LibraryDialog.svelte';
	let { library, creating = $bindable(false) }: { library: LibraryWorkspace; creating?: boolean } =
		$props();
	let editing = $state<Collection | null>(null);
	let collectionName = $state('');
	let removal = $state<
		{ kind: 'notes'; ids: string[] } | { kind: 'collection'; collection: Collection } | null
	>(null);
	let targetCollection = $state('');
	const activeCollection = $derived(
		library.snapshot.collections.find((collection) => collection.id === library.collectionId)
	);
	const allSelected = $derived(
		library.visible.length > 0 &&
			library.visible.every((document) => library.selected.includes(document.id))
	);
	const selection = $derived(
		library.snapshot.documents.filter((document) => library.selected.includes(document.id))
	);
	const allFavorites = $derived(
		selection.length > 0 && selection.every((document) => document.favorite)
	);

	function closeEditor() {
		creating = false;
		editing = null;
		collectionName = '';
	}
	async function saveCollection() {
		await library.service.saveCollection(collectionName, editing?.id);
		await library.refresh();
	}
	async function confirmRemoval() {
		if (!removal) return;
		if (removal.kind === 'notes') await library.service.deleteDocuments(removal.ids);
		else await library.service.deleteCollection(removal.collection.id);
		await library.refresh();
	}
	async function downloadOriginal() {
		const document = selection[0];
		if (!document) return;
		await library.perform(async () =>
			downloadBlob(await library.service.openDocument(document.id), document.filename)
		);
	}
</script>

<div class="actions" aria-label="Library actions">
	<label class="selection"
		><input
			type="checkbox"
			aria-label="Select all visible notes"
			checked={allSelected}
			disabled={!library.visible.length}
			onchange={() =>
				(library.selected = allSelected ? [] : library.visible.map((document) => document.id))}
		/>{library.selected.length ? `${library.selected.length} selected` : 'Select notes'}</label
	>
	{#if library.selected.length}
		<button
			onclick={() =>
				void library.perform(() =>
					library.service.setFavorite([...library.selected], !allFavorites)
				)}>{allFavorites ? 'Unfavorite' : 'Favorite'}</button
		>
		<select
			aria-label="Add selected notes to collection"
			bind:value={targetCollection}
			onchange={() => {
				if (targetCollection)
					void library.perform(() =>
						library.service.setMembership([...library.selected], targetCollection, true)
					);
				targetCollection = '';
			}}
		>
			<option value="">Add to collection…</option>
			{#each library.snapshot.collections as collection}<option value={collection.id}
					>{collection.name}</option
				>{/each}
		</select>
		{#if activeCollection}<button
				onclick={() =>
					void library.perform(() =>
						library.service.setMembership([...library.selected], activeCollection!.id, false)
					)}>Remove from collection</button
			>{/if}
		{#if library.selected.length === 1}<button onclick={() => void downloadOriginal()}
				>Download original</button
			>{/if}
		<button onclick={() => (removal = { kind: 'notes', ids: [...library.selected] })}
			>Delete from library</button
		>
		<button onclick={() => (library.selected = [])}>Clear selection</button>
	{:else if activeCollection}
		<button
			onclick={() => {
				editing = activeCollection;
				collectionName = activeCollection.name;
			}}>Rename collection</button
		>
		<button onclick={() => (removal = { kind: 'collection', collection: activeCollection })}
			>Delete collection</button
		>
	{/if}
</div>

{#if creating || editing}
	<LibraryDialog
		title={editing ? 'Rename collection' : 'Create collection'}
		description="Organize notes without making extra copies."
		confirmLabel="Save collection"
		onConfirm={saveCollection}
		onClose={closeEditor}
	>
		<label class="block text-xs"
			>Collection name<input
				class="mt-2 w-full rounded border border-subtle bg-bg px-3 py-2 text-text"
				required
				maxlength="120"
				bind:value={collectionName}
			/></label
		>
	</LibraryDialog>
{/if}
{#if removal}
	<LibraryDialog
		title={removal.kind === 'notes' ? 'Delete notes from library?' : 'Delete collection?'}
		destructive
		description={removal.kind === 'notes'
			? `Delete ${removal.ids.length} saved ${removal.ids.length === 1 ? 'note' : 'notes'} and their collection memberships from this browser? Files on your device are unaffected.`
			: `Delete “${removal.collection.name}”? Its notes will remain in All notes.`}
		confirmLabel={removal.kind === 'notes' ? 'Delete notes' : 'Delete collection'}
		onConfirm={confirmRemoval}
		onClose={() => (removal = null)}
	/>
{/if}

<style>
	.actions {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 8px;
		padding: 10px 24px;
		border-bottom: 1px solid var(--site-border);
		font-size: 11px;
	}
	.selection {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-right: 6px;
		color: var(--site-muted);
	}
	button,
	select {
		border: 1px solid var(--site-border);
		border-radius: 4px;
		padding: 5px 8px;
		background: var(--site-bg);
		color: var(--site-text);
		cursor: pointer;
	}
	button:hover {
		background: var(--site-surface);
	}
	input {
		accent-color: var(--color-accent);
	}
	@media (max-width: 720px) {
		.actions {
			padding: 10px 16px;
		}
	}
</style>
