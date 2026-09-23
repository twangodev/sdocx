<script lang="ts">
	import { Download, LoaderCircle, X } from '@lucide/svelte';
	import IconButton from './IconButton.svelte';
	import type { ArchiveKind, PngScale as Scale } from '$converter/document-session.svelte';
	interface ExportDialogModel {
		exporting: boolean;
		rendering: boolean;
		pngScale: Scale;
	}

	interface ExportDialogActions {
		onScale: (scale: Scale) => void;
		onCurrentSvg: () => void;
		onCurrentPng: () => void;
		onArchive: (kind: ArchiveKind) => void;
		onJson: () => void;
		onCancel: () => void;
	}

	interface Props {
		model: ExportDialogModel;
		actions: ExportDialogActions;
	}

	let { model, actions }: Props = $props();

	let dialog: HTMLDialogElement;
	let format = $state<'svg' | 'png' | 'json' | 'everything'>('svg');
	let scope = $state('current');
	let scale = $state<Scale>(1);
	const documentExport = $derived(format === 'json' || format === 'everything');
	const includesPng = $derived(format === 'png' || format === 'everything');

	function open(): void {
		scale = model.pngScale;
		dialog.showModal();
	}

	function download(): void {
		if (model.exporting || model.rendering) return;
		actions.onScale(scale);
		dialog.close();
		if (format === 'json') actions.onJson();
		else if (format === 'everything') actions.onArchive('everything');
		else if (scope === 'all') actions.onArchive(format);
		else if (format === 'png') actions.onCurrentPng();
		else actions.onCurrentSvg();
	}
</script>

{#if model.exporting}
	<IconButton label="Cancel export" tooltip tone="danger" onclick={actions.onCancel}>
		<LoaderCircle class="animate-spin" size={13} strokeWidth={1.4} />
	</IconButton>
{:else}
	<IconButton label="Export document" tooltip onclick={open}>
		<Download size={13} strokeWidth={1.4} />
	</IconButton>
{/if}

<dialog bind:this={dialog} aria-labelledby="export-title" aria-describedby="export-description" class="m-auto w-[min(26rem,calc(100%-2rem))] max-h-[calc(100svh-2rem)] overflow-auto rounded-xl border border-subtle bg-raised p-5 text-text shadow-xl backdrop:bg-black/35">
	<div class="mb-2 flex items-center justify-between gap-3">
		<h2 id="export-title" class="flex items-center gap-2 text-base font-semibold"><Download size={17} />Export document</h2>
		<IconButton label="Close export dialog" onclick={() => dialog.close()}><X size={15} /></IconButton>
	</div>
	<p id="export-description" class="mb-5 text-xs text-muted">Save a copy to your device. Exports use the current document color mode.</p>
	<form onsubmit={(event) => { event.preventDefault(); download(); }} class="grid gap-4">
		<label class="grid gap-1.5 text-xs font-medium">Format
			<select aria-label="Format" bind:value={format} class="rounded-md border border-subtle bg-bg p-2 text-text">
				<option value="svg">SVG · scalable vector image</option>
				<option value="png">PNG · image</option>
				<option value="json">JSON · document structure</option>
				<option value="everything">Everything · SVG, PNG & JSON (.zip)</option>
			</select>
		</label>
		{#if !documentExport}
			<label class="grid gap-1.5 text-xs font-medium">Pages
				<select aria-label="Pages" bind:value={scope} class="rounded-md border border-subtle bg-bg p-2 text-text">
					<option value="current">Current page</option>
					<option value="all">All pages (.zip)</option>
				</select>
			</label>
		{:else}
			<p class="text-xs text-muted">Includes the whole document.</p>
		{/if}
		{#if includesPng}
			<label class="grid gap-1.5 text-xs font-medium">PNG resolution
				<select aria-label="PNG resolution" bind:value={scale} class="rounded-md border border-subtle bg-bg p-2 text-text">
					<option value={1}>1× · original size</option>
					<option value={2}>2× · higher resolution</option>
				</select>
			</label>
		{/if}
		{#if model.rendering}<p class="text-xs text-muted" role="status">Finishing the preview…</p>{/if}
		<div class="mt-1 flex justify-end gap-2 border-t border-subtle pt-4">
			<button type="button" class="rounded-md px-3 py-2 text-xs hover:bg-surface" onclick={() => dialog.close()}>Cancel</button>
			<button type="submit" disabled={model.rendering || model.exporting} class="inline-flex items-center gap-2 rounded-md bg-accent px-3 py-2 text-xs font-medium text-white disabled:opacity-50"><Download size={14} />Download</button>
		</div>
	</form>
</dialog>
