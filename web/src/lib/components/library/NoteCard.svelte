<script lang="ts">
	import { Ellipsis, Star } from '@lucide/svelte';
	import type { LibraryDocument } from '$lib/library/model';
	import type { AssetStore } from '$lib/library/asset-store';
	import { noteMenu, noteTitle, type NoteAction } from '$lib/library/note-menu';
	import { formatStorageBytes } from '$lib/library/browser-storage';
	import DropdownMenu from '../ui/DropdownMenu.svelte';
	import IconButton from '../ui/IconButton.svelte';
	import SelectionCheckbox from '../ui/SelectionCheckbox.svelte';
	import NoteThumbnail from './NoteThumbnail.svelte';
	let {
		document,
		assets,
		view,
		selected,
		onSelect,
		onAction
	}: {
		document: LibraryDocument;
		assets: AssetStore;
		view: 'grid' | 'list';
		selected: boolean;
		onSelect: () => void;
		onAction: (action: NoteAction) => void;
	} = $props();
	const title = $derived(noteTitle(document));
</script>

{#snippet selection()}
	<SelectionCheckbox
		label={`Select ${document.title}`}
		checked={selected}
		onCheckedChange={onSelect}
	/>
{/snippet}
{#snippet menu()}
	<DropdownMenu items={noteMenu(document)} {onAction} align="end">
		{#snippet children({ props })}
			<IconButton {...props} label={`Actions for ${document.title}`} size={7}
				><Ellipsis size={15} /></IconButton
			>
		{/snippet}
	</DropdownMenu>
{/snippet}

<article class="note" class:row={view === 'list'} class:selected aria-label={document.title}>
	{#if view === 'list'}<div class="row-selection">{@render selection()}</div>{/if}
	<button
		class="open-note"
		aria-label={`Open ${document.title}`}
		title={document.filename}
		onclick={() => onAction('open')}
	>
		<div class="preview"><NoteThumbnail name={document.thumbnail} {assets} /></div>
		<span class="note-title">{title}</span>
	</button>
	{#if view === 'grid'}
		<div class="note-footer">
			<span class="metadata mr-auto"
				>{document.pageCount} {document.pageCount === 1 ? 'page' : 'pages'}</span
			>
			{#if document.favorite}<Star
					size={11}
					fill="currentColor"
					class="text-muted"
					aria-label="Favorite"
				/>{/if}
			<div class="secondary">{@render selection()}</div>
			<div class="secondary">{@render menu()}</div>
		</div>
	{:else}
		<span class="row-pages metadata"
			>{document.pageCount} {document.pageCount === 1 ? 'page' : 'pages'}</span
		>
		<span class="row-size metadata">{formatStorageBytes(document.size)}</span>
		<div class="flex items-center justify-end">{@render menu()}</div>
	{/if}
</article>

<style>
	.note {
		min-width: 0;
		border: 1px solid var(--site-border);
		border-radius: 5px;
		background: var(--site-bg);
		transition: border-color 120ms;
	}
	.note:hover {
		border-color: var(--site-muted);
	}
	.note.selected {
		border-color: var(--color-accent);
	}
	.open-note {
		display: block;
		width: 100%;
		min-width: 0;
		padding: 5px 5px 0;
		cursor: pointer;
		text-align: left;
		border-radius: 4px;
	}
	.preview {
		aspect-ratio: 1;
		padding: 8px;
		overflow: hidden;
		background: var(--site-surface);
		border-radius: 3px;
	}
	.note-title {
		display: block;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		padding: 9px 5px 0;
		font-size: 12px;
		font-weight: 500;
	}
	.note-footer {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 2px 5px 5px 10px;
	}
	.metadata {
		color: var(--site-muted);
		font-size: 11px;
		white-space: nowrap;
	}
	.secondary {
		opacity: 0;
		transition: opacity 120ms;
	}
	.note:hover .secondary,
	.note:focus-within .secondary,
	.note.selected .secondary,
	.note:has(:global([aria-expanded='true'])) .secondary {
		opacity: 1;
	}
	.row {
		display: grid;
		grid-template-columns: var(--note-columns);
		align-items: center;
		gap: 12px;
		height: 56px;
		padding: 0 8px;
		border: 0;
		border-radius: 0;
		border-bottom: 1px solid var(--site-border);
	}
	.row:last-child {
		border-bottom: 0;
	}
	.row:hover,
	.row.selected {
		background: var(--site-surface);
	}
	.row.selected {
		box-shadow: inset 2px 0 var(--color-accent);
	}
	.row .open-note {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 0;
	}
	.row .preview {
		flex-shrink: 0;
		width: 34px;
		height: 40px;
		padding: 2px;
	}
	.row .note-title {
		padding: 0;
	}
	.row-pages,
	.row-size {
		text-align: right;
		font-variant-numeric: tabular-nums;
	}
	@media (hover: none) {
		.secondary {
			opacity: 1;
		}
	}
	@media (max-width: 720px) {
		.row-size {
			display: none;
		}
		.row {
			gap: 6px;
		}
		.row .open-note {
			gap: 8px;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.secondary,
		.note {
			transition: none;
		}
	}
</style>
