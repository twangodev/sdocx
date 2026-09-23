<script lang="ts">
	import { ArrowLeft, Download, LoaderCircle, Star, X } from '@lucide/svelte';
	import { tick } from 'svelte';
	import IconButton from './IconButton.svelte';
	import type { ArchiveKind, PngScale as Scale } from '$converter/document-session.svelte';
	interface ExportDialogModel {
		exporting: boolean;
		rendering: boolean;
		pngScale: Scale;
	}

	interface ExportDialogActions {
		onScale: (scale: Scale) => void;
		onCurrentSvg: () => Promise<string>;
		onCurrentPng: () => Promise<string>;
		onPdf: (allPages: boolean) => Promise<string>;
		onArchive: (kind: ArchiveKind) => Promise<string>;
		onJson: () => Promise<string>;
		onCancel: () => void;
	}

	interface Props {
		model: ExportDialogModel;
		actions: ExportDialogActions;
	}

	let { model, actions }: Props = $props();

	let dialog: HTMLDialogElement;
	let format = $state<'svg' | 'png' | 'pdf' | 'json' | 'everything'>('svg');
	let step = $state<'configure' | 'exporting' | 'complete'>('configure');
	let error = $state('');
	let starLink = $state<HTMLAnchorElement>();
	let formatSelect = $state<HTMLSelectElement>();
	let scope = $state('current');
	let scale = $state<Scale>(1);
	const documentExport = $derived(format === 'json' || format === 'everything');
	const includesPng = $derived(format === 'png' || format === 'everything');

	function open(): void {
		scale = model.pngScale;
		step = 'configure';
		error = '';
		dialog.showModal();
	}

	async function backToExport(): Promise<void> {
		step = 'configure';
		await tick();
		formatSelect?.focus();
	}

	async function download(): Promise<void> {
		if (model.exporting || model.rendering || step === 'exporting') return;
		actions.onScale(scale);
		step = 'exporting';
		error = '';
		try {
			if (format === 'json') error = await actions.onJson();
			else if (format === 'everything') error = await actions.onArchive('everything');
			else if (format === 'pdf') error = await actions.onPdf(scope === 'all');
			else if (scope === 'all') error = await actions.onArchive(format);
			else if (format === 'png') error = await actions.onCurrentPng();
			else error = await actions.onCurrentSvg();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Export failed. Please try again.';
		}
		step = error ? 'configure' : 'complete';
		await tick();
		if (dialog.open && step === 'complete') starLink?.focus();
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
		<h2 id="export-title" class="flex items-center gap-2 text-base font-semibold">{#if step === 'complete'}<Star size={17} />Thanks for using sdocx{:else}<Download size={17} />Export document{/if}</h2>
		<IconButton label="Close export dialog" onclick={() => dialog.close()}><X size={15} /></IconButton>
	</div>
	{#if step === 'complete'}
		<p id="export-description" class="mt-3 text-sm">Your download is ready.</p>
		<p class="mt-2 text-xs leading-relaxed text-muted">If sdocx helped you, give the project a star on GitHub. It helps others discover this open-source Samsung Notes converter.</p>
		<div class="mt-5 flex items-center gap-2">
			<button type="button" class="mr-auto inline-flex items-center gap-1.5 py-2 text-xs text-muted hover:text-text" onclick={backToExport}><ArrowLeft size={13} class="shrink-0" />Back to export</button>
			<button type="button" class="rounded-md px-3 py-2 text-xs hover:bg-surface" onclick={() => dialog.close()}>Done</button>
			<a bind:this={starLink} href="https://github.com/twangodev/sdocx" target="_blank" rel="noreferrer" class="inline-flex items-center gap-2 rounded-md bg-accent px-3 py-2 text-xs font-medium text-white"><Star size={14} />Star on GitHub</a>
		</div>
	{:else}
	<p id="export-description" class="mb-5 text-xs text-muted">Save a copy to your device. Exports use the current document color mode.</p>
	<form onsubmit={(event) => { event.preventDefault(); void download(); }} class="grid gap-4">
		<fieldset disabled={step === 'exporting'} class="grid gap-4">
		<label class="grid gap-1.5 text-xs font-medium">Format
			<select bind:this={formatSelect} aria-label="Format" bind:value={format} class="rounded-md border border-subtle bg-bg p-2 text-text">
				<option value="svg">SVG · scalable vector image</option>
				<option value="png">PNG · image</option>
				<option value="pdf">PDF · vector document</option>
				<option value="json">JSON · document structure</option>
				<option value="everything">Everything · SVG, PNG & JSON (.zip)</option>
			</select>
		</label>
		{#if !documentExport}
			<label class="grid gap-1.5 text-xs font-medium">Pages
				<select aria-label="Pages" bind:value={scope} class="rounded-md border border-subtle bg-bg p-2 text-text">
					<option value="current">Current page</option>
					<option value="all">All pages{format === 'pdf' ? ' (.pdf)' : ' (.zip)'}</option>
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
		</fieldset>
		{#if error}<p role="alert" class="text-xs text-danger">{error}</p>{/if}
		{#if model.rendering}<p class="text-xs text-muted" role="status">Finishing the preview…</p>{/if}
		<div class="mt-1 flex justify-end gap-2 border-t border-subtle pt-4">
			<button type="button" class="rounded-md px-3 py-2 text-xs hover:bg-surface" onclick={() => dialog.close()}>{step === 'exporting' ? 'Hide' : 'Cancel'}</button>
			<button type="submit" disabled={model.rendering || model.exporting || step === 'exporting'} class="inline-flex items-center gap-2 rounded-md bg-accent px-3 py-2 text-xs font-medium text-white disabled:opacity-50">{#if step === 'exporting'}<LoaderCircle size={14} class="animate-spin" />Exporting…{:else}<Download size={14} />Download{/if}</button>
		</div>
	</form>
	{/if}
</dialog>
