<script lang="ts">
	import { Download, LoaderCircle, Star, X, ChevronLeft, ChevronRight, FileJson } from '@lucide/svelte';
	import { tick } from 'svelte';
	import ExportPreview from './ExportPreview.svelte';
	import ProcessingProgress from './ProcessingProgress.svelte';
	import ColorModeSwitch from './ColorModeSwitch.svelte';
	import type { ColorMode, PageRenderReport, SvgRenderResult } from '$converter/protocol';
	import type { ProcessingProgress as WorkProgress, WorkListener } from '$converter/progress';
	import RenderNotices from '../RenderNotices.svelte';
	import IconButton from './IconButton.svelte';
	import { exportDetails, type ExportFormat, type ExportRequest } from '$converter/export-options';
	import { sanitizeStem } from '$converter/files';

	interface Props {
		model: {
			title: string;
			filename: string;
			pageCount: number;
			pageIndex: number;
			colorMode: ColorMode;
			exporting: boolean;
			rendering: boolean;
			exportProgress: string;
			exportWork?: WorkProgress | null;
			exportReports: PageRenderReport[];
			exportColorMode: ColorMode;
		};
		actions: {
			onExport: (request: ExportRequest) => Promise<string>;
			onResolvePages: (selection: string) => Promise<number[]>;
			onPreview: (page: number, mode: ColorMode, onProgress?: WorkListener) => Promise<SvgRenderResult>;
			onCancel: () => void;
		};
	}

	let { model, actions }: Props = $props();
	let dialog: HTMLDialogElement;
	let formatSelect: HTMLSelectElement;
	let format = $state<ExportFormat>('pdf');
	let opened = $state(false);
	let colorMode = $state<ColorMode>('auto');
	let previewPage = $state(0);
	let scope = $state<'all' | 'current' | 'range'>('all');
	let range = $state('');
	let scale = $state<1 | 2>(1);
	let currentPage = $state(0);
	let rangeIndices = $state<number[]>([]);
	let resolvedRange = $state<string | null>(null);
	let validating = $state(false);
	let rangeError = $state('');
	let error = $state('');
	let submitting = $state(false);
	let downloaded = $state(false);
	const wholeDocument = $derived(format === 'json' || format === 'everything');
	const includesPng = $derived(format === 'png' || format === 'everything');
	const custom = $derived(!wholeDocument && model.pageCount > 1 && scope === 'range');
	const pageIndices = $derived(wholeDocument || model.pageCount === 1 || scope === 'all'
		? Array.from({ length: model.pageCount }, (_, index) => index)
		: scope === 'current' ? [currentPage] : rangeIndices);
	const details = $derived(exportDetails({ format, pageIndices, pngScale: scale }, model.pageCount, sanitizeStem(model.filename)));
	const busy = $derived(submitting || model.exporting);
	const validSelection = $derived(pageIndices.length > 0 && (!custom || (!validating && resolvedRange === range && !rangeError)));

	$effect(() => {
		// Editing settings clears the previous result, without hiding the form.
		format; scope; range; scale; colorMode;
		downloaded = false;
		error = '';
	});

	$effect(() => {
		const selection = range;
		const pageCount = model.pageCount;
		resolvedRange = null;
		rangeIndices = [];
		rangeError = '';
		validating = custom;
		if (!custom) return;
		let active = true;
		const timer = setTimeout(async () => {
			try {
				const indices = await actions.onResolvePages(selection);
				if (!active || pageCount !== model.pageCount) return;
				rangeIndices = indices;
				resolvedRange = selection;
			} catch (cause) {
				if (active) rangeError = cause instanceof Error ? cause.message : 'Could not validate these pages.';
			} finally {
				if (active) validating = false;
			}
		}, 150);
		return () => { active = false; clearTimeout(timer); };
	});

	$effect(() => {
		if (validSelection && !pageIndices.includes(previewPage)) previewPage = pageIndices[0];
	});
	const previewPosition = $derived(pageIndices.indexOf(previewPage));
	function stepPreview(direction: -1 | 1) {
		const page = pageIndices[previewPosition + direction];
		if (page !== undefined && !busy) previewPage = page;
	}

	async function open(): Promise<void> {
		format = 'pdf';
		scope = 'all';
		range = '';
		scale = 1;
		currentPage = model.pageIndex;
		previewPage = model.pageIndex;
		colorMode = model.colorMode;
		opened = true;
		downloaded = false;
		error = '';
		dialog.showModal();
		await tick();
		formatSelect.focus();
	}

	async function download(): Promise<void> {
		if (busy || model.rendering || !validSelection) return;
		const request: ExportRequest = { format, pageIndices: [...pageIndices], pngScale: scale, colorMode };
		submitting = true;
		downloaded = false;
		error = '';
		try {
			error = await actions.onExport(request);
			downloaded = !error;
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Export failed. Please try again.';
		} finally {
			submitting = false;
		}
	}
</script>

<IconButton
	label={model.exporting ? 'Cancel export' : 'Export document'}
	tooltip
	tone={model.exporting ? 'danger' : 'default'}
	onclick={() => model.exporting ? actions.onCancel() : void open()}
>
	{#if model.exporting}
		<LoaderCircle class="animate-spin" size={13} strokeWidth={1.4} />
	{:else}
		<Download size={13} strokeWidth={1.4} />
	{/if}
</IconButton>

<dialog bind:this={dialog} onclose={() => opened = false} aria-labelledby="export-title" aria-describedby="export-description" class="export-dialog m-auto rounded-xl border border-subtle bg-raised text-text shadow-xl backdrop:bg-black/35">
	<header class="px-5 pt-5"><div class="flex items-center justify-between gap-3">
		<h2 id="export-title" class="flex items-center gap-2 text-base font-semibold"><Download size={17} />Export document</h2>
		<IconButton label="Close export dialog" onclick={() => dialog.close()}><X size={15} /></IconButton>
	</div>
	<p id="export-description" class="mt-1 mb-5 truncate text-xs text-muted" title={model.title}>{model.title} · {model.pageCount} {model.pageCount === 1 ? 'page' : 'pages'}</p></header>
	<form onsubmit={(event) => { event.preventDefault(); void download(); }} class="export-form">
		<div class="export-body">
			<section class="export-preview" aria-label="Export preview">
				{#if format === 'json'}
					<div class="flex flex-1 flex-col items-center justify-center gap-3 bg-canvas p-8 text-center">
						<FileJson size={36} strokeWidth={1} class="text-muted" />
						<p class="font-medium">Document structure</p>
						<p class="max-w-64 text-xs leading-relaxed text-muted">All {model.pageCount} pages and their document data in one JSON file.</p>
					</div>
				{:else}
					<div class="flex min-h-11 items-center justify-between gap-2 border-b border-subtle px-3 text-xs">
						<span class="text-muted">Page preview</span>
						<div class="flex items-center gap-2">
							<IconButton label="Previous export preview page" disabled={busy || !validSelection || previewPosition <= 0} onclick={() => stepPreview(-1)}><ChevronLeft size={14} /></IconButton>
							<span data-export-preview-page>{validSelection ? `Page ${previewPage + 1} · ${previewPosition + 1} of ${pageIndices.length} selected` : 'No pages selected'}</span>
							<IconButton label="Next export preview page" disabled={busy || !validSelection || previewPosition >= pageIndices.length - 1} onclick={() => stepPreview(1)}><ChevronRight size={14} /></IconButton>
						</div>
					</div>
					{#if opened}
						<ExportPreview pageIndex={previewPage} {colorMode} {busy} enabled={validSelection && !model.rendering} pngScale={includesPng ? scale : undefined} render={actions.onPreview} />
					{/if}
				{/if}
			</section>
			<div class="export-settings">
				<fieldset disabled={busy} class="grid min-w-0 gap-4">
					<label class="grid gap-1.5 text-xs font-medium">Format
						<select bind:this={formatSelect} aria-label="Format" bind:value={format} class="w-full rounded-md border border-subtle bg-bg p-2 text-text">
							<option value="pdf">PDF · document</option>
							<option value="png">PNG · image</option>
							<option value="svg">SVG · vector image</option>
							<option value="json">JSON · document structure</option>
							<option value="everything">SVG + PNG + JSON (ZIP)</option>
						</select>
					</label>
					{#if wholeDocument}
						<p class="text-xs text-muted">Includes the whole document.</p>
					{:else if model.pageCount === 1}
						<p class="text-xs text-muted">1 page</p>
					{:else}
						<fieldset class="grid gap-2 text-xs">
							<legend class="mb-2 font-medium">Pages</legend>
							<label class="flex items-center gap-2 py-1"><input type="radio" name="export-pages" value="all" bind:group={scope} class="accent-accent" />All pages · {model.pageCount}</label>
							<label class="flex items-center gap-2 py-1"><input type="radio" name="export-pages" value="current" bind:group={scope} class="accent-accent" />Current page · {currentPage + 1}</label>
							<label class="flex items-center gap-2 py-1"><input type="radio" name="export-pages" value="range" bind:group={scope} class="accent-accent" />Custom range</label>
							{#if scope === 'range'}
								<input aria-label="Page range" aria-describedby="range-help" aria-invalid={Boolean(rangeError)} bind:value={range} placeholder="1–3, 5" class="w-full rounded-md border border-subtle bg-bg p-2 text-text" />
								<p id="range-help" class={rangeError ? 'text-danger' : 'text-muted'}>{rangeError || 'Use commas and ranges, for example 1–3, 5.'}</p>
							{/if}
						</fieldset>
					{/if}
					{#if format !== 'json'}
						<div class="grid gap-2 text-xs"><span class="font-medium">Export colors</span>
							<div class="flex items-center justify-between"><span class="text-muted">{colorMode === 'auto' ? 'Automatic' : colorMode === 'dark' ? 'Dark' : 'Light'}</span><ColorModeSwitch value={colorMode} disabled={busy} onChange={(mode) => colorMode = mode} /></div>
							<p class="text-[11px] text-muted">Applies to this download only.</p>
						</div>
					{/if}
					{#if includesPng}
						<label class="grid gap-1.5 text-xs font-medium">PNG resolution
							<select aria-label="PNG resolution" bind:value={scale} class="rounded-md border border-subtle bg-bg p-2 text-text">
								<option value={1}>1× · original size</option>
								<option value={2}>2× · higher resolution</option>
							</select>
						</label>
					{/if}
				</fieldset>
				<div class="rounded-md bg-surface p-3 text-xs" aria-live="polite">
					{#if validSelection}
						<p class="break-all font-medium" data-export-filename>{details.filename}</p>
						<p class="mt-1 text-muted">{details.description}</p>
					{:else}
						<p class="text-muted">{validating ? 'Checking pages…' : 'Choose valid pages to see the output.'}</p>
					{/if}
				</div>
			</div>
		</div>
		<footer class="export-footer">
			{#if error}<p role="alert" class="text-xs text-danger">{error}</p>{/if}
			{#if busy}
				<p role="status" class="text-xs text-muted">{model.exportProgress || 'Preparing download'}</p>
				<ProcessingProgress progress={model.exportWork} />
			{:else if downloaded}
				<div class="max-h-32 overflow-auto"><RenderNotices reports={model.exportReports} label={`Download rendering (${model.exportColorMode})`} /></div>
				<div class="flex flex-wrap items-center justify-between gap-2 text-xs">
					<p role="status">Download started</p>
					<a href="https://github.com/twangodev/sdocx" target="_blank" rel="noreferrer" class="inline-flex items-center gap-1 text-muted hover:text-text"><Star size={12} />Star on GitHub</a>
				</div>
			{:else if model.rendering}
				<p role="status" class="text-xs text-muted">Finishing the preview…</p>
			{/if}
			<div class="flex justify-end gap-2">
				<button type="button" class="rounded-md px-3 py-2 text-xs hover:bg-surface" onclick={() => dialog.close()}>{busy ? 'Hide' : 'Close'}</button>
				<button type="submit" disabled={busy || model.rendering || !validSelection} class="inline-flex items-center gap-2 rounded-md bg-accent px-3 py-2 text-xs font-medium text-white disabled:opacity-50">{#if busy}<LoaderCircle size={14} class="animate-spin" />Exporting…{:else}<Download size={14} />{details.button}{/if}</button>
			</div>
		</footer>
	</form>
</dialog>

<style>
	.export-dialog { width: min(64rem, calc(100% - 2rem)); max-height: calc(100svh - 2rem); padding: 0; overflow: hidden; }
	.export-form { display: flex; flex-direction: column; min-height: 0; }
	.export-body { display: grid; grid-template-columns: minmax(0, 1fr) 19rem; height: min(65svh, 38rem); min-height: 22rem; border-top: 1px solid var(--site-border); }
	.export-preview { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
	.export-settings { display: flex; flex-direction: column; gap: 20px; padding: 20px; overflow: auto; border-left: 1px solid var(--site-border); }
	.export-footer { display: grid; gap: 12px; padding: 16px 20px; border-top: 1px solid var(--site-border); }
	@media (max-width: 720px) {
		.export-dialog { width: calc(100% - 1rem); }
		.export-body { display: flex; flex-direction: column; height: auto; min-height: 0; max-height: calc(100svh - 12rem); overflow: auto; }
		.export-preview { flex: none; height: clamp(15rem, 36svh, 22rem); }
		.export-settings { flex: none; border-left: 0; border-top: 1px solid var(--site-border); overflow: visible; }
	}
	@media (max-height: 600px) and (min-width: 721px) {
		.export-body { min-height: 0; height: calc(100svh - 12rem); }
	}
</style>
