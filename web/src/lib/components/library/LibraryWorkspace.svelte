<script lang="ts">
	import { onMount } from 'svelte';
	import { FilePlus2, Folder, Grid2X2, List, Menu, X } from '@lucide/svelte';
	import type { LibraryWorkspace } from '$lib/library/workspace.svelte';
	import type { LibrarySource } from '$lib/library/model';
	import { visibleDocuments } from '$lib/library/view';
	import NoteThumbnail from './NoteThumbnail.svelte';
	import ImportStatus from './ImportStatus.svelte';
	let { library, onImport, onOpen, onTemporary }: {
		library: LibraryWorkspace; onImport: () => void; onOpen: (id: string) => void; onTemporary: (file: File) => void;
	} = $props();
	let sidebarOpen = $state(false);
	let scroller: HTMLDivElement;
	const sources = [{ id: 'all', label: 'All notes' }, { id: 'recent', label: 'Recent' }, { id: 'favorites', label: 'Favorites' }] as const;
	function navigate(source: LibrarySource) { library.selectSource(source); sidebarOpen = false; if (scroller) scroller.scrollTop = 0; }
	onMount(() => { scroller.scrollTop = library.scrollTop; });
</script>

<section class="library" aria-label="Notes library">
	{#if sidebarOpen}<button class="backdrop" aria-label="Close library navigation" onclick={() => sidebarOpen = false}></button>{/if}
	<aside class:expanded={sidebarOpen} aria-label="Library sidebar">
		<div class="sidebar-heading">library <button class="mobile" aria-label="Close navigation" onclick={() => sidebarOpen = false}><X size={16} /></button></div>
		<nav aria-label="Library">
			{#each sources as source}
				<button class:active={library.source === source.id} onclick={() => navigate(source.id)}>
					<span>{source.label}</span><span class="count">{visibleDocuments(library.snapshot, source.id, '', 'newest').length}</span>
				</button>
			{/each}
		</nav>
		<div class="sidebar-heading collections-heading">collections</div>
		<nav aria-label="Collections">
			{#each library.snapshot.collections as collection (collection.id)}
				<button class:active={library.collectionId === collection.id} onclick={() => navigate({ collectionId: collection.id })}><Folder size={14} /><span class="collection-name">{collection.name}</span><span class="count">{library.snapshot.memberships.filter((membership) => membership.collectionId === collection.id).length}</span></button>
			{/each}
			{#if !library.snapshot.collections.length}<p class="empty-collections">No collections yet.</p>{/if}
		</nav>
		<p class="storage-caption">Saved in this browser</p>
	</aside>
	<div class="main">
		<header>
			<button class="mobile control" aria-label="Library navigation" onclick={() => sidebarOpen = true}><Menu size={16} /></button>
			<h1>{library.title}</h1>
			<button class="control import" disabled={library.loading || library.importing} onclick={onImport}><FilePlus2 size={15} />Import notes</button>
		</header>
		<div class="tools">
			<input aria-label="Search notes" placeholder="Search notes…" type="search" bind:value={library.search} />
			<select aria-label="Sort notes" bind:value={library.sort}><option value="newest">Newest imports</option><option value="title">Title</option></select>
			<div class="view-buttons"><button class="control" aria-label="Grid view" aria-pressed={library.view === 'grid'} onclick={() => library.view = 'grid'}><Grid2X2 size={15} /></button><button class="control" aria-label="List view" aria-pressed={library.view === 'list'} onclick={() => library.view = 'list'}><List size={15} /></button></div>
		</div>
		<ImportStatus {library} {onTemporary} />
		{#if library.error}<p role="alert" class="error">{library.error}</p>{/if}
		<div class="notes-scroll" bind:this={scroller} onscroll={() => library.scrollTop = scroller.scrollTop}>
			{#if library.loading}<p class="empty">Loading library…</p>
			{:else if !library.visible.length}
				<div class="empty">
					<h2>{library.snapshot.documents.length ? 'No matching notes' : 'Your notes, in one place'}</h2>
					<p class="lede">{library.snapshot.documents.length ? 'Try another search or collection.' : 'Import Samsung Notes files to start your library. Files stay in this browser.'}</p>
					{#if !library.snapshot.documents.length}<button class="control" disabled={library.importing} onclick={onImport}>Import .sdocx files</button>{/if}
				</div>
			{:else}
				<div class="notes" class:list={library.view === 'list'}>
					{#each library.visible as document (document.id)}
						<article class="note" aria-label={document.title}>
							<button class="preview" aria-label={`Open ${document.title}`} onclick={() => onOpen(document.id)}><NoteThumbnail name={document.thumbnail} assets={library.service.assets} /></button>
							<div class="note-info"><button class="note-title" onclick={() => onOpen(document.id)}>{document.title}</button><p title={document.filename}>{document.filename}</p><p>{document.pageCount} {document.pageCount === 1 ? 'page' : 'pages'} · {(document.size / 1024).toFixed(0)} KiB</p></div>
						</article>
					{/each}
				</div>
			{/if}
		</div>
	</div>
</section>

<style>
	.library { display: flex; width: 100%; height: calc(100svh - 2.5rem); min-height: 0; font-size: 12px; }
	aside { width: 208px; flex-shrink: 0; border-right: 1px solid var(--site-border); padding: 20px 10px; display: flex; flex-direction: column; overflow-y: auto; }
	.sidebar-heading { display: flex; justify-content: space-between; align-items: center; padding: 0 10px 10px; color: var(--site-muted); font-size: 11px; letter-spacing: .04em; }
	.collections-heading { margin-top: 22px; border-top: 1px solid var(--site-border); padding-top: 18px; }
	nav button { display: flex; align-items: center; gap: 8px; width: 100%; text-align: left; padding: 8px 10px; border-radius: 4px; }
	nav button:hover, nav button.active, .control:hover, .control[aria-pressed='true'] { background: var(--site-surface); }
	.count { margin-left: auto; color: var(--site-muted); font-size: 10px; }
	.collection-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.empty-collections { color: var(--site-muted); padding: 4px 10px; font-size: 11px; }
	.storage-caption { margin-top: auto; padding: 24px 10px 0; color: var(--site-muted); font-size: 10px; }
	.main { flex: 1; min-width: 0; display: flex; flex-direction: column; }
	header { display: flex; align-items: center; gap: 12px; padding: 20px 24px 14px; }
	h1 { font-size: 18px; font-weight: 550; }
	.control { display: inline-flex; align-items: center; justify-content: center; gap: 7px; border: 1px solid var(--site-border); border-radius: 4px; padding: 7px 10px; }
	button { cursor: pointer; } button:disabled { opacity: .45; cursor: default; }
	button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; }
	.import { margin-left: auto; }
	.tools { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; padding: 0 24px 16px; border-bottom: 1px solid var(--site-border); }
	input, select { border: 1px solid var(--site-border); background: var(--site-bg); color: inherit; border-radius: 4px; padding: 7px 10px; }
	input { flex: 1; min-width: 120px; } .view-buttons { display: flex; gap: 4px; }
	.notes-scroll { overflow-y: auto; min-height: 0; flex: 1; padding: 24px; }
	.notes { display: grid; grid-template-columns: repeat(auto-fill, minmax(175px, 1fr)); gap: 20px; }
	.note { border: 1px solid var(--site-border); border-radius: 5px; overflow: hidden; min-width: 0; }
	.preview { display: block; width: 100%; height: 200px; padding: 12px; background: var(--site-surface); }
	.note-info { padding: 12px; min-width: 0; } .note-title { display: block; max-width: 100%; font-weight: 550; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.note-info p { color: var(--site-muted); font-size: 10px; margin-top: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.list { display: flex; flex-direction: column; gap: 8px; } .list .note { display: flex; align-items: center; } .list .preview { width: 64px; height: 80px; flex-shrink: 0; padding: 6px; }
	.empty { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 14px; min-height: 300px; text-align: center; color: var(--site-muted); } .empty h2 { font-size: 20px; color: var(--site-text); } .lede { max-width: 340px; line-height: 1.7; }
	.error { padding: 12px 24px; color: var(--site-text); border-bottom: 1px solid var(--site-border); }
	.mobile { display: none; } .backdrop { position: fixed; inset: 40px 0 0; z-index: 59; background: #0006; }
	@media(max-width: 720px) { aside { display: none; } aside.expanded { display: flex; position: fixed; inset: 40px auto 0 0; z-index: 60; background: var(--site-bg); width: 240px; } .mobile { display: inline-flex; } header { padding: 16px; } .tools { padding: 0 16px 12px; } .notes-scroll { padding: 16px; } .notes { grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); gap: 12px; } .preview { height: 170px; } }
</style>
