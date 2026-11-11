<script lang="ts">
	import { Ellipsis, X } from '@lucide/svelte';
	import { separator, type MenuLeaf } from '$lib/menu';
	import type { LibraryWorkspace } from '$lib/library/workspace.svelte';
	import type { LibraryDialogIntent } from '$lib/library/dialog-intent';
	import CompactSelectMenu from '../ui/CompactSelectMenu.svelte';
	import DropdownMenu from '../ui/DropdownMenu.svelte';
	import IconButton from '../ui/IconButton.svelte';
	import LibraryDialog from './LibraryDialog.svelte';
	let { library, dialog = $bindable(), onCreateCollection }: {
		library: LibraryWorkspace;
		dialog: LibraryDialogIntent | null;
		onCreateCollection: () => void;
	} = $props();
	const activeCollection = $derived(
		library.snapshot.collections.find((collection) => collection.id === library.collectionId)
	);
	const selection = $derived(
		library.snapshot.documents.filter((document) => library.selected.includes(document.id))
	);
	const allFavorites = $derived(
		selection.length > 0 && selection.every((document) => document.favorite)
	);
	type Action = 'favorite' | 'download' | 'remove-membership' | 'delete';
	const actions = $derived.by(() => {
		const items: MenuLeaf<Action>[] = [
			{ kind: 'action', label: allFavorites ? 'Unfavorite' : 'Favorite', action: 'favorite' }
		];
		if (selection.length === 1)
			items.push({ kind: 'action', label: 'Download original', action: 'download' });
		if (activeCollection)
			items.push({ kind: 'action', label: 'Remove from collection', action: 'remove-membership' });
		items.push(separator(), {
			kind: 'action',
			label: 'Delete from library',
			action: 'delete',
			tone: 'danger'
		});
		return items;
	});
	const collectionItems = $derived<MenuLeaf<string>[]>([
		...library.snapshot.collections.map((collection) => ({
			kind: 'action' as const,
			label: collection.name,
			action: collection.id
		})),
		separator(),
		{ kind: 'action', label: 'New collection…', action: 'create' }
	]);
	function closeDialog() {
		dialog = null;
	}
	async function saveCollection() {
		const intent = dialog;
		if (!intent || (intent.kind !== 'create-collection' && intent.kind !== 'rename-collection')) return;
		const { collectionId, name } = intent;
		const documentIds = intent.kind === 'create-collection' ? [...intent.documentIds] : [];
		await library.service.saveCollection(name, collectionId);
		if (documentIds.length) await library.service.setMembership(documentIds, collectionId, true);
		await library.refresh();
	}
	async function confirmRemoval() {
		const intent = dialog;
		if (intent?.kind === 'delete-notes') await library.service.deleteDocuments([...intent.documentIds]);
		else if (intent?.kind === 'delete-collection') await library.service.deleteCollection(intent.collectionId);
		else return;
		await library.refresh();
	}
	function runAction(action: Action) {
		const ids = [...library.selected];
		if (action === 'delete') dialog = { kind: 'delete-notes', documentIds: ids };
		else if (action === 'download' && ids[0]) void library.downloadOriginal(ids[0]);
		else if (action === 'favorite')
			void library.perform(() => library.service.setFavorite(ids, !allFavorites));
		else if (action === 'remove-membership' && activeCollection) {
			const collectionId = activeCollection.id;
			void library.perform(() => library.service.setMembership(ids, collectionId, false));
		}
	}
</script>

{#if library.selected.length}
	<div class="flex min-w-0 flex-1 items-center gap-1" aria-label="Library actions">
		<span class="mr-auto whitespace-nowrap text-xs font-medium"
			>{library.selected.length} selected</span
		>
		<CompactSelectMenu
			label="Add selected notes to collection"
			value="Add to"
			items={collectionItems}
			onAction={(id) => {
				if (id === 'create') onCreateCollection();
				else
					void library.perform(() =>
						library.service.setMembership([...library.selected], id, true)
					);
			}}
		/>
		<CompactSelectMenu
			label="Selection actions"
			value="Actions"
			items={actions}
			onAction={runAction}
		/>
		<IconButton label="Clear selection" tooltip size={7} onclick={() => (library.selected = [])}
			><X size={14} /></IconButton
		>
	</div>
{:else if activeCollection}
	<DropdownMenu
		items={[
			{ kind: 'action', label: 'Rename collection', action: 'rename' },
			{ kind: 'action', label: 'Delete collection', action: 'delete', tone: 'danger' }
		]}
		onAction={(action) => {
			const collection = activeCollection;
			if (!collection) return;
			dialog = { kind: action === 'rename' ? 'rename-collection' : 'delete-collection',
				collectionId: collection.id, name: collection.name };
		}}
	>
		{#snippet children({ props })}<IconButton {...props} label="Collection actions" size={7}
				><Ellipsis size={15} /></IconButton
			>{/snippet}
	</DropdownMenu>
{/if}

{#if dialog?.kind === 'create-collection' || dialog?.kind === 'rename-collection'}
	<LibraryDialog
		title={dialog.kind === 'rename-collection' ? 'Rename collection' : 'Create collection'}
		description="Organize notes without making extra copies."
		confirmLabel="Save collection"
		onConfirm={saveCollection}
		onClose={closeDialog}
	>
		<label class="block text-xs"
			>Collection name<input
				class="mt-2 w-full rounded border border-subtle bg-bg px-3 py-2 text-text"
				required
				maxlength="120"
				bind:value={dialog.name}
			/></label
		>
	</LibraryDialog>
{:else if dialog}
	<LibraryDialog
		title={dialog.kind === 'delete-notes' ? 'Delete notes from library?' : 'Delete collection?'}
		destructive
		description={dialog.kind === 'delete-notes'
			? `Delete ${dialog.documentIds.length} saved ${dialog.documentIds.length === 1 ? 'note' : 'notes'} and their collection memberships from this browser? Files on your device are unaffected.`
			: `Delete “${dialog.name}”? Its notes will remain in All notes.`}
		confirmLabel={dialog.kind === 'delete-notes' ? 'Delete notes' : 'Delete collection'}
		onConfirm={confirmRemoval}
		onClose={closeDialog}
	/>
{/if}
