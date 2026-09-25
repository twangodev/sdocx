<script lang="ts">
	import { Clock3, Files, Folder, FolderPlus, HardDrive, Star } from '@lucide/svelte';
	import IconButton from '../ui/IconButton.svelte';
	import type { LibraryWorkspace } from '$lib/library/workspace.svelte';
	import type { LibrarySource } from '$lib/library/model';
	import { visibleDocuments } from '$lib/library/view';
	let {
		library,
		onNavigate,
		onCreate,
		onStorage
	}: {
		library: LibraryWorkspace;
		onNavigate: (source: LibrarySource) => void;
		onCreate: () => void;
		onStorage: () => void;
	} = $props();
	const sources = [
		{ id: 'all', label: 'All notes', icon: Files },
		{ id: 'recent', label: 'Recent', icon: Clock3 },
		{ id: 'favorites', label: 'Favorites', icon: Star }
	] as const;
</script>

<div class="flex h-full min-h-0 flex-col px-2 py-3">
	<p class="px-2 pb-2 text-[11px] text-muted">library</p>
	<nav aria-label="Library" class="space-y-0.5">
		{#each sources as source}
			<button
				class="source"
				class:active={library.source === source.id}
				aria-current={library.source === source.id ? 'page' : undefined}
				onclick={() => onNavigate(source.id)}
			>
				<source.icon size={13} strokeWidth={1.5} />
				<span class="min-w-0 flex-1 truncate">{source.label}</span>
				<span class="count"
					>{visibleDocuments(library.snapshot, source.id, '', 'newest').length}</span
				>
			</button>
		{/each}
	</nav>
	<div class="mt-4 flex h-10 items-center justify-between border-t border-subtle pr-1 pl-2">
		<span class="text-[11px] text-muted">collections</span>
		<IconButton
			label="Create collection"
			tooltip
			size={7}
			disabled={!library.available}
			onclick={onCreate}><FolderPlus size={13} strokeWidth={1.5} /></IconButton
		>
	</div>
	<nav aria-label="Collections" class="min-h-0 flex-1 space-y-0.5 overflow-y-auto">
		{#each library.snapshot.collections as collection (collection.id)}
			<button
				class="source"
				class:active={library.collectionId === collection.id}
				aria-current={library.collectionId === collection.id ? 'page' : undefined}
				onclick={() => onNavigate({ collectionId: collection.id })}
			>
				<Folder size={13} strokeWidth={1.5} />
				<span class="min-w-0 flex-1 truncate">{collection.name}</span>
				<span class="count"
					>{library.snapshot.memberships.filter(
						(membership) => membership.collectionId === collection.id
					).length}</span
				>
			</button>
		{/each}
		{#if !library.snapshot.collections.length}<p class="px-2 py-2 text-[11px] text-muted">
				No collections yet
			</p>{/if}
	</nav>
	<div class="mt-3 border-t border-subtle pt-2">
		<button class="source" onclick={onStorage}
			><HardDrive size={13} strokeWidth={1.5} /><span>Browser storage</span></button
		>
	</div>
</div>

<style>
	.source {
		display: flex;
		height: 32px;
		width: 100%;
		align-items: center;
		gap: 8px;
		padding: 0 8px;
		border-radius: 4px;
		color: var(--site-muted);
		font-size: 12px;
		text-align: left;
		cursor: pointer;
	}
	.source:hover,
	.source.active {
		background: var(--site-surface);
		color: var(--site-text);
	}
	.count {
		font-family: var(--font-mono);
		font-size: 10px;
		color: var(--site-muted);
	}
</style>
