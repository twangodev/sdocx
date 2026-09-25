<script lang="ts">
	import { onMount } from 'svelte';
	import { Dialog } from 'bits-ui';
	import { FilePlus2, Grid2X2, List, Menu, Search, X } from '@lucide/svelte';
	import type { LibraryWorkspace } from '$lib/library/workspace.svelte';
	import type { LibraryDocument, LibrarySort, LibrarySource } from '$lib/library/model';
	import type { NoteAction } from '$lib/library/note-menu';
	import type { MenuLeaf } from '$lib/menu';
	import Button from '../ui/Button.svelte';
	import IconButton from '../ui/IconButton.svelte';
	import CompactSelectMenu from '../ui/CompactSelectMenu.svelte';
	import SegmentedControl from '../ui/SegmentedControl.svelte';
	import SelectionCheckbox from '../ui/SelectionCheckbox.svelte';
	import NoteCard from './NoteCard.svelte';
	import LibrarySidebar from './LibrarySidebar.svelte';
	import ImportStatus from './ImportStatus.svelte';
	import LibraryActions from './LibraryActions.svelte';
	import StorageDialog from './StorageDialog.svelte';

	let {
		library,
		onImport,
		onOpen,
		onTemporary
	}: {
		library: LibraryWorkspace;
		onImport: () => void;
		onOpen: (id: string) => void;
		onTemporary: (file: File) => void;
	} = $props();
	let sidebarOpen = $state(false);
	let creatingCollection = $state(false);
	let removingNotes = $state<string[] | null>(null);
	let storageOpen = $state(false);
	let scroller: HTMLDivElement;
	const hasNotes = $derived(library.snapshot.documents.length > 0);
	const allSelected = $derived(
		library.visible.length > 0 &&
			library.visible.every((document) => library.selected.includes(document.id))
	);
	const someSelected = $derived(
		library.visible.some((document) => library.selected.includes(document.id))
	);
	const sorts: MenuLeaf<LibrarySort>[] = [
		{ kind: 'action', label: 'Newest imports', action: 'newest' },
		{ kind: 'action', label: 'Title', action: 'title' }
	];

	function navigate(source: LibrarySource) {
		library.selectSource(source);
		sidebarOpen = false;
		if (scroller) scroller.scrollTop = 0;
	}
	function createCollection() {
		sidebarOpen = false;
		creatingCollection = true;
	}
	function openStorage() {
		sidebarOpen = false;
		storageOpen = true;
	}
	function noteAction(document: LibraryDocument, action: NoteAction) {
		if (action === 'open') onOpen(document.id);
		else if (action === 'favorite')
			void library.perform(() => library.service.setFavorite([document.id], !document.favorite));
		else if (action === 'download') void library.downloadOriginal(document.id);
		else removingNotes = [document.id];
	}
	onMount(() => {
		scroller.scrollTop = library.scrollTop;
		const desktop = matchMedia('(min-width: 721px)');
		const closeDrawer = () => {
			if (desktop.matches) sidebarOpen = false;
		};
		desktop.addEventListener('change', closeDrawer);
		return () => desktop.removeEventListener('change', closeDrawer);
	});
</script>

{#snippet sidebar()}
	<LibrarySidebar
		{library}
		onNavigate={navigate}
		onCreate={createCollection}
		onStorage={openStorage}
	/>
{/snippet}

<section class="library" aria-label="Notes library">
	<aside class="desktop-sidebar" aria-label="Library sidebar">{@render sidebar()}</aside>
	<div class="main">
		<header class="library-toolbar" class:has-selection={library.selected.length > 0}>
			<Dialog.Root bind:open={sidebarOpen}>
				<div class="mobile-trigger">
					<Dialog.Trigger>
						{#snippet child({ props })}<IconButton {...props} label="Library navigation" size={8}
								><Menu size={16} /></IconButton
							>{/snippet}
					</Dialog.Trigger>
				</div>
				<Dialog.Portal>
					<Dialog.Overlay class="fixed inset-0 z-60 bg-black/45" />
					<Dialog.Content
						class="fixed inset-y-0 left-0 z-70 flex w-64 max-w-[85vw] flex-col border-r border-subtle bg-bg"
					>
						<div
							class="flex h-12 shrink-0 items-center justify-between border-b border-subtle px-3"
						>
							<Dialog.Title class="text-sm font-medium">Your library</Dialog.Title>
							<Dialog.Close
								>{#snippet child({ props })}<IconButton
										{...props}
										label="Close library navigation"
										size={8}><X size={16} /></IconButton
									>{/snippet}</Dialog.Close
							>
						</div>
						<Dialog.Description class="sr-only"
							>Browse notes and collections or manage browser storage.</Dialog.Description
						>
						<div class="min-h-0 flex-1">{@render sidebar()}</div>
					</Dialog.Content>
				</Dialog.Portal>
			</Dialog.Root>
			{#if hasNotes}
				<SelectionCheckbox
					label="Select all visible notes"
					checked={allSelected}
					indeterminate={someSelected && !allSelected}
					disabled={!library.visible.length}
					onCheckedChange={() =>
						(library.selected = allSelected ? [] : library.visible.map((document) => document.id))}
				/>
			{/if}
			<h1 class:sr-only={library.selected.length > 0}>{library.title}</h1>
			<LibraryActions {library} bind:creating={creatingCollection} bind:removing={removingNotes} />
			{#if hasNotes && !library.selected.length}
				<div class="browse-controls">
					<label class="search-field">
						<Search size={13} strokeWidth={1.5} aria-hidden="true" />
						<input
							aria-label="Search notes"
							placeholder="Search notes"
							type="search"
							bind:value={library.search}
						/>
					</label>
					<CompactSelectMenu
						label="Sort notes"
						value={library.sort === 'newest' ? 'Newest' : 'Title'}
						items={sorts.map((item) =>
							item.kind === 'action' ? { ...item, checked: item.action === library.sort } : item
						)}
						onAction={(sort) => (library.sort = sort)}
					/>
					<SegmentedControl
						options={['grid', 'list'] as const}
						bind:value={library.view}
						label="Library view"
						itemLabel={(view) => (view === 'grid' ? 'Grid view' : 'List view')}
						class="w-14"
					>
						{#snippet item(view)}{#if view === 'grid'}<Grid2X2 size={12} />{:else}<List
									size={12}
								/>{/if}{/snippet}
					</SegmentedControl>
				</div>
				<Button
					size={7}
					tone="primary"
					class="import-button"
					disabled={library.loading || library.importing}
					onclick={onImport}><FilePlus2 size={13} strokeWidth={1.5} />Import notes</Button
				>
			{/if}
		</header>
		<ImportStatus {library} {onTemporary} />
		{#if library.error}<p role="alert" class="border-b border-subtle px-4 py-3 text-xs">
				{library.error}
			</p>{/if}
		<div
			class="notes-scroll"
			bind:this={scroller}
			onscroll={() => (library.scrollTop = scroller.scrollTop)}
		>
			{#if library.loading}<p class="empty text-muted">Loading library…</p>
			{:else if !library.visible.length}
				<div class="empty">
					{#if !hasNotes}<div
							class="mb-4 grid size-10 place-items-center rounded border border-subtle bg-bg text-muted"
						>
							<FilePlus2 size={18} strokeWidth={1.25} />
						</div>{/if}
					<h2>{hasNotes ? 'No matching notes' : 'Your notes, in one place'}</h2>
					<p class="lede">
						{hasNotes
							? 'Try another search or collection.'
							: 'Import Samsung Notes files to start your library. Files stay in this browser.'}
					</p>
					{#if !hasNotes}<Button
							class="mt-4"
							tone="primary"
							disabled={library.importing}
							onclick={onImport}>Import notes</Button
						>{/if}
				</div>
			{:else}
				<div class="notes" class:list={library.view === 'list'}>
					{#if library.view === 'list'}<div class="list-heading" aria-hidden="true">
							<span></span><span>Name</span><span class="text-right">Pages</span><span
								class="list-size text-right">Size</span
							><span></span>
						</div>{/if}
					{#each library.visible as document (document.id)}
						<NoteCard
							{document}
							assets={library.service.assets}
							view={library.view}
							selected={library.selected.includes(document.id)}
							onSelect={() => library.toggleSelection(document.id)}
							onAction={(action) => noteAction(document, action)}
						/>
					{/each}
				</div>
			{/if}
		</div>
	</div>
</section>
{#if storageOpen}<StorageDialog {library} onClose={() => (storageOpen = false)} />{/if}

<style>
	.library {
		display: flex;
		width: 100%;
		height: calc(100svh - 2.5rem);
		min-height: 0;
		--note-columns: 28px minmax(0, 1fr) 5rem 5rem 28px;
	}
	.desktop-sidebar {
		width: 192px;
		flex-shrink: 0;
		border-right: 1px solid var(--site-border);
	}
	.main {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
	}
	.library-toolbar {
		display: flex;
		align-items: center;
		gap: 8px;
		min-height: 48px;
		flex-shrink: 0;
		padding: 8px 12px;
		border-bottom: 1px solid var(--site-border);
		background: var(--site-bg);
	}
	h1 {
		min-width: 0;
		max-width: 200px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 13px;
		font-weight: 550;
	}
	.browse-controls {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-left: auto;
		min-width: 0;
	}
	.search-field {
		display: flex;
		align-items: center;
		gap: 6px;
		height: 28px;
		min-width: 96px;
		width: 220px;
		padding: 0 8px;
		color: var(--site-muted);
		background: var(--site-surface);
		border: 1px solid var(--site-border);
		border-radius: 4px;
	}
	.search-field:focus-within {
		border-color: var(--color-accent);
	}
	.search-field input {
		width: 100%;
		min-width: 0;
		outline: none;
		background: transparent;
		font-size: 11px;
		color: var(--site-text);
	}
	.notes-scroll {
		min-height: 0;
		flex: 1;
		overflow-y: auto;
		padding: 16px;
		background: var(--site-canvas);
	}
	.notes {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
		gap: 12px;
	}
	.notes.list {
		display: block;
		overflow: hidden;
		border: 1px solid var(--site-border);
		border-radius: 5px;
		background: var(--site-bg);
	}
	.list-heading {
		display: grid;
		grid-template-columns: var(--note-columns);
		gap: 12px;
		padding: 8px;
		border-bottom: 1px solid var(--site-border);
		color: var(--site-muted);
		font-size: 11px;
	}
	.empty {
		display: flex;
		min-height: 100%;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
	}
	.empty h2 {
		font-size: 14px;
		font-weight: 500;
	}
	.lede {
		margin-top: 6px;
		max-width: 290px;
		font-size: 12px;
		line-height: 1.6;
		color: var(--site-muted);
	}
	.mobile-trigger {
		display: none;
	}
	@media (max-width: 1000px) {
		.search-field {
			width: 150px;
		}
	}
	@media (max-width: 850px) {
		.library-toolbar {
			flex-wrap: wrap;
		}
		.browse-controls {
			width: 100%;
			order: 1;
			margin: 0;
			padding-top: 2px;
		}
		.search-field {
			flex: 1;
			width: auto;
		}
		.library-toolbar :global(.import-button) {
			margin-left: auto;
		}
	}
	@media (max-width: 720px) {
		.library {
			--note-columns: 28px minmax(0, 1fr) 4rem 28px;
		}
		.desktop-sidebar {
			display: none;
		}
		.mobile-trigger {
			display: block;
		}
		.library-toolbar {
			padding: 8px;
			gap: 4px;
		}
		.library-toolbar.has-selection {
			flex-wrap: nowrap;
		}
		h1 {
			max-width: 140px;
		}
		.notes-scroll {
			padding: 12px;
		}
		.notes {
			grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
		}
		.list-heading {
			gap: 6px;
		}
		.list-size {
			display: none;
		}
	}
</style>
