<script lang="ts">
	import { Ellipsis, X } from '@lucide/svelte';
	import { separator, type MenuLeaf } from '$lib/menu';
	import type { LibraryWorkspace } from '$lib/library/workspace.svelte';
	import type { Collection } from '$lib/library/model';
	import CompactSelectMenu from '../ui/CompactSelectMenu.svelte';
	import DropdownMenu from '../ui/DropdownMenu.svelte';
	import IconButton from '../ui/IconButton.svelte';
	import LibraryDialog from './LibraryDialog.svelte';
	let {
		library,
		creating = $bindable(false),
		removing = $bindable<string[] | null>(null)
	}: {
		library: LibraryWorkspace;
		creating?: boolean;
		removing?: string[] | null;
	} = $props();
	let editing = $state<Collection | null>(null);
	let collectionName = $state('');
	let removingCollection = $state<Collection | null>(null);
	const removal = $derived(
		removing
			? { kind: 'notes' as const, ids: removing }
			: removingCollection
				? { kind: 'collection' as const, collection: removingCollection }
				: null
	);
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
	function closeEditor() {
		creating = false;
		editing = null;
		collectionName = '';
	}
	function closeRemoval() {
		removing = null;
		removingCollection = null;
	}
	async function saveCollection() {
		const collection = await library.service.saveCollection(collectionName, editing?.id);
		if (!editing && library.selected.length)
			await library.service.setMembership([...library.selected], collection.id, true);
		await library.refresh();
	}
	async function confirmRemoval() {
		if (!removal) return;
		if (removal.kind === 'notes') await library.service.deleteDocuments(removal.ids);
		else await library.service.deleteCollection(removal.collection.id);
		await library.refresh();
	}
	function runAction(action: Action) {
		const ids = [...library.selected];
		if (action === 'delete') removing = ids;
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
				if (id === 'create') creating = true;
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
			if (action === 'rename') {
				editing = activeCollection!;
				collectionName = activeCollection!.name;
			} else removingCollection = activeCollection!;
		}}
	>
		{#snippet children({ props })}<IconButton {...props} label="Collection actions" size={7}
				><Ellipsis size={15} /></IconButton
			>{/snippet}
	</DropdownMenu>
{/if}

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
		onClose={closeRemoval}
	/>
{/if}
