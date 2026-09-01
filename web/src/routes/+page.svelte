<script lang="ts">
	import { onMount } from 'svelte';
	import { FolderOpen } from '@lucide/svelte';
	import ConverterToolbar from '$lib/components/ConverterToolbar.svelte';
	import DocumentHeader from '$lib/components/DocumentHeader.svelte';
	import DocumentInfoPanel from '$lib/components/DocumentInfoPanel.svelte';
	import ExportPanel from '$lib/components/ExportPanel.svelte';
	import { ConverterClient } from '$converter/client';
	import {
		assertAcceptedFile,
		isLargeFile,
		type ColorMode,
		type DocumentSummary,
		type WorkerPhase
	} from '$converter/protocol';
	import {
		createExportManifest,
		createZip,
		downloadBlob,
		pageFilename,
		sanitizeStem,
		svgToPng,
		textBytes
	} from '$converter/files';
	import { toInspectionView, type InspectionView } from '$converter/view-model';

	let picker = $state<HTMLInputElement>();
	let client: ConverterClient;
	let activeFile = $state<File | null>(null);
	let summary = $state<DocumentSummary | null>(null);
	let details = $state<InspectionView | null>(null);
	let pageIndex = $state(0);
	let colorMode = $state<ColorMode>('auto');
	let pngScale = $state<1 | 2>(1);
	let previewZoom = $state(100);
	let fitPage = $state(true);
	let previewUrls = $state<string[]>([]);
	let previewScroller = $state<HTMLDivElement>();
	let phase = $state<WorkerPhase | null>(null);
	let status = $state('Waiting for a document');
	let error = $state('');
	let parsing = $state(false);
	let rendering = $state(false);
	let exporting = $state(false);
	let dragging = $state(false);
	let exportProgress = $state('');
	let renderGeneration = 0;
	let dragDepth = 0;

	const hasDocument = $derived(summary !== null && activeFile !== null);
	const stem = $derived(activeFile ? sanitizeStem(activeFile.name) : 'document');
	const zoomSteps = [50, 75, 100, 125, 150, 175, 200];

	onMount(() => {
		client = new ConverterClient((nextPhase, message) => {
			phase = nextPhase;
			status = message;
		});

		return () => {
			client.destroy();
			releasePreviews();
		};
	});

	function releasePreviews(): void {
		for (const url of previewUrls) {
			if (url) URL.revokeObjectURL(url);
		}
		previewUrls = [];
	}

	function clearDocument(): void {
		renderGeneration += 1;
		releasePreviews();
		activeFile = null;
		summary = null;
		details = null;
		pageIndex = 0;
		previewZoom = 100;
		fitPage = true;
	}

	async function closeDocument(): Promise<void> {
		try {
			await client.dispose();
		} finally {
			clearDocument();
			phase = null;
			status = 'Waiting for a document';
		}
	}

	async function loadFile(file: File): Promise<void> {
		error = '';
		try {
			assertAcceptedFile(file);
		} catch (cause) {
			error = messageFrom(cause);
			status = 'Could not open document';
			return;
		}

		try {
			if (
				isLargeFile(file) &&
				!window.confirm(
					'This file is over 100 MiB. Parsing may use substantial memory. Continue locally?'
				)
			) {
				return;
			}

			clearDocument();
			activeFile = file;
			parsing = true;
			status = 'Reading file from this device';
			const bytes = await file.arrayBuffer();
			const nextSummary = await client.load(bytes);
			summary = nextSummary;
			details = toInspectionView(nextSummary.inspection);
			pageIndex = 0;
			status = `${nextSummary.pageCount} ${nextSummary.pageCount === 1 ? 'page' : 'pages'} ready`;
			phase = 'ready';

			if (nextSummary.pageCount > 0) await renderPreviews();
		} catch (cause) {
			clearDocument();
			error = messageFrom(cause);
			status = 'Could not open document';
		} finally {
			parsing = false;
		}
	}

	async function renderPreviews(): Promise<void> {
		if (!summary || summary.pageCount === 0) return;
		const generation = ++renderGeneration;
		const pageCount = summary.pageCount;
		rendering = true;
		error = '';
		releasePreviews();
		previewUrls = Array(pageCount).fill('');
		try {
			for (let index = 0; index < pageCount; index += 1) {
				status = `Rendering page ${index + 1} of ${pageCount}`;
				const svg = await client.renderPage(index, colorMode);
				if (generation !== renderGeneration) return;
				const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
				previewUrls[index] = url;
				previewUrls = [...previewUrls];
			}
			status = `${pageCount} ${pageCount === 1 ? 'page' : 'pages'} rendered locally`;
		} catch (cause) {
			if (generation === renderGeneration) error = messageFrom(cause);
		} finally {
			if (generation === renderGeneration) rendering = false;
		}
	}

	function selectPage(nextPage: number): void {
		const page = previewScroller?.querySelector<HTMLElement>(`[data-page-index="${nextPage}"]`);
		page?.scrollIntoView({ behavior: 'auto', block: 'start' });
		pageIndex = nextPage;
	}

	function stepPage(direction: -1 | 1): void {
		if (!summary) return;
		selectPage(Math.min(summary.pageCount - 1, Math.max(0, pageIndex + direction)));
	}

	function alignSelectedPage(): void {
		const selectedPage = pageIndex;
		requestAnimationFrame(() => selectPage(selectedPage));
	}

	function setZoom(zoom: number): void {
		fitPage = false;
		previewZoom = zoom;
		alignSelectedPage();
	}

	function stepZoom(direction: -1 | 1): void {
		const currentZoom = fitPage ? 100 : previewZoom;
		const currentIndex = zoomSteps.indexOf(currentZoom);
		const nextIndex = Math.min(
			zoomSteps.length - 1,
			Math.max(0, (currentIndex === -1 ? zoomSteps.indexOf(100) : currentIndex) + direction)
		);
		setZoom(zoomSteps[nextIndex]);
	}

	function fitPreviewWidth(): void {
		setZoom(100);
	}

	function fitPreviewPage(): void {
		fitPage = true;
		alignSelectedPage();
	}

	async function selectColorMode(nextMode: ColorMode): Promise<void> {
		colorMode = nextMode;
		await renderPreviews();
	}

	function updatePageFromScroll(event: Event): void {
		const scroller = event.currentTarget as HTMLDivElement;
		const anchor = scroller.getBoundingClientRect().top + scroller.clientHeight * 0.3;
		const pages = scroller.querySelectorAll<HTMLElement>('[data-page-index]');
		let nextPage = pageIndex;

		for (const page of pages) {
			const bounds = page.getBoundingClientRect();
			const index = Number(page.dataset.pageIndex);
			if (bounds.top <= anchor && bounds.bottom > anchor) {
				nextPage = index;
				break;
			}
			if (bounds.top > anchor) {
				nextPage = index;
				break;
			}
		}

		if (nextPage !== pageIndex) pageIndex = nextPage;
	}

	function onFileInput(event: Event): void {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		if (file) void loadFile(file);
		input.value = '';
	}

	function carriesFiles(event: DragEvent): boolean {
		return Array.from(event.dataTransfer?.types ?? []).includes('Files');
	}

	function onWindowDragEnter(event: DragEvent): void {
		if (!carriesFiles(event)) return;
		event.preventDefault();
		dragDepth += 1;
		dragging = true;
	}

	function onWindowDragOver(event: DragEvent): void {
		if (!carriesFiles(event)) return;
		event.preventDefault();
		if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy';
		dragging = true;
	}

	function onWindowDragLeave(event: DragEvent): void {
		if (!dragging) return;
		event.preventDefault();
		dragDepth = Math.max(0, dragDepth - 1);
		if (dragDepth === 0) dragging = false;
	}

	function onWindowDrop(event: DragEvent): void {
		const fileDrop = carriesFiles(event);
		if (fileDrop) event.preventDefault();
		dragDepth = 0;
		dragging = false;

		const file = event.dataTransfer?.files[0];
		if (fileDrop && file) void loadFile(file);
	}

	function cancel(): void {
		client.cancel();
		clearDocument();
		parsing = false;
		rendering = false;
		exporting = false;
		exportProgress = '';
		phase = null;
		status = 'Processing cancelled';
	}

	async function downloadCurrentSvg(): Promise<void> {
		await withExport(async () => {
			const svg = await client.renderPage(pageIndex, colorMode);
			downloadBlob(new Blob([svg], { type: 'image/svg+xml' }), pageFilename(stem, pageIndex, 'svg'));
		});
	}

	async function downloadCurrentPng(): Promise<void> {
		await withExport(async () => {
			const svg = await client.renderPage(pageIndex, colorMode);
			const png = await svgToPng(svg, pngScale);
			downloadBlob(png, pageFilename(stem, pageIndex, 'png'));
		});
	}

	async function downloadJson(): Promise<void> {
		await withExport(async () => {
			const json = await client.exportJson();
			downloadBlob(new Blob([json], { type: 'application/json' }), `${stem}.json`);
		});
	}

	type ArchiveKind = 'svg' | 'png' | 'everything';

	async function downloadArchive(kind: ArchiveKind): Promise<void> {
		if (!summary || !activeFile) return;
		await withExport(async () => {
			const pageCount = summary!.pageCount;
			const sourceName = activeFile!.name;
			const inspectionJson = kind === 'everything' ? await client.exportJson() : '';
			const manifest = createExportManifest(sourceName, pageCount, colorMode, pngScale);

			async function* entries(): AsyncGenerator<{ name: string; bytes: Uint8Array }> {
				if (kind === 'everything') {
					yield { name: 'document.json', bytes: textBytes(inspectionJson) };
					yield {
						name: 'manifest.json',
						bytes: textBytes(JSON.stringify(manifest, null, 2))
					};
				}

				for (let index = 0; index < pageCount; index += 1) {
					exportProgress = `Rendering page ${index + 1} of ${pageCount}`;
					const svg = await client.renderPage(index, colorMode);
					if (kind === 'svg' || kind === 'everything') {
						yield { name: pageFilename(stem, index, 'svg'), bytes: textBytes(svg) };
					}
					if (kind === 'png' || kind === 'everything') {
						exportProgress = `Rasterizing page ${index + 1} of ${pageCount}`;
						const png = await svgToPng(svg, pngScale);
						yield {
							name: pageFilename(stem, index, 'png'),
							bytes: new Uint8Array(await png.arrayBuffer())
						};
					}
				}
			}

			const archive = await createZip(entries());
			downloadBlob(archive, `${stem}-${kind}.zip`);
		});
	}

	async function withExport(task: () => Promise<void>): Promise<void> {
		exporting = true;
		error = '';
		exportProgress = 'Preparing download';
		try {
			await task();
			status = 'Download ready';
		} catch (cause) {
			error = messageFrom(cause);
		} finally {
			exporting = false;
			exportProgress = '';
		}
	}

	function messageFrom(cause: unknown): string {
		return cause instanceof Error ? cause.message : 'The document could not be processed.';
	}

</script>

<svelte:head>
	<title>sdocx — local Samsung Notes converter</title>
	<meta
		name="description"
		content="Inspect and export Samsung Notes .sdocx documents entirely in your browser."
	/>
</svelte:head>

<svelte:window
	ondragenter={onWindowDragEnter}
	ondragover={onWindowDragOver}
	ondragleave={onWindowDragLeave}
	ondrop={onWindowDrop}
/>

{#if dragging}
	<div class="drop-overlay" role="status" aria-live="polite">
		<div class="drop-overlay-copy">
			<strong>{hasDocument ? 'drop to replace document' : 'drop .sdocx to open'}</strong>
			<span>release anywhere · processed locally</span>
		</div>
	</div>
{/if}

{#if !hasDocument}
	<section class="intro" aria-labelledby="intro-title">
		<h1 id="intro-title">Open a Samsung Notes file</h1>
		<p class="lede">
			Preview and export .sdocx documents. Files stay in this browser.
		</p>

		<button
			type="button"
			class="drop-zone"
			onclick={() => picker?.click()}
		>
			{#if !parsing}<FolderOpen size={13} strokeWidth={1.4} />{/if}
			{parsing ? status : 'open .sdocx'}
		</button>
		<p class="drop-hint">{parsing ? 'processing locally' : 'or drop one here · max 250 MiB'}</p>

		<input
			bind:this={picker}
			class="visually-hidden"
			type="file"
			accept=".sdocx,application/zip"
			onchange={onFileInput}
		/>
		{#if parsing}
			<button class="cancel-link" type="button" onclick={cancel}>cancel parsing</button>
		{/if}
		{#if error}<p class="error" role="alert">{error}</p>{/if}
	</section>
{:else if summary && activeFile}
	<section class="workspace" aria-label="Document converter">
		<DocumentHeader
			title={details?.title || activeFile.name}
			filename={activeFile.name}
			fileSize={activeFile.size}
			pageCount={summary.pageCount}
			onReplace={() => picker?.click()}
			onClose={() => void closeDocument()}
		/>

		<input
			bind:this={picker}
			class="visually-hidden"
			type="file"
			accept=".sdocx,application/zip"
			onchange={onFileInput}
		/>

		<div class="work-grid">
			<DocumentInfoPanel pageCount={summary.pageCount} {details} />

			<div class="preview-panel">
				<ConverterToolbar
					{pageIndex}
					pageCount={summary.pageCount}
					{previewZoom}
					{fitPage}
					{colorMode}
					disabled={exporting || rendering}
					onSelectPage={selectPage}
					onStepPage={stepPage}
					onSetZoom={setZoom}
					onStepZoom={stepZoom}
					onFitWidth={fitPreviewWidth}
					onFitPage={fitPreviewPage}
					onColorMode={(nextMode) => void selectColorMode(nextMode)}
				/>
				<div
					bind:this={previewScroller}
					class="canvas-wrap"
					aria-busy={rendering}
					onscroll={updatePageFromScroll}
				>
					{#if previewUrls.length}
						<div
							class="page-stack"
							class:fit-page={fitPage}
							data-zoom={fitPage ? 'page' : previewZoom}
							style:width={fitPage ? '100%' : `${previewZoom}%`}
						>
							{#each previewUrls as url, index}
								<figure
									class:active={pageIndex === index}
									data-page-index={index}
								>
									{#if url}
										<img src={url} alt={`Rendered preview of page ${index + 1}`} />
									{:else}
										<div class="page-placeholder">
											<span class="spinner" aria-hidden="true"></span>
										</div>
									{/if}
									<figcaption>page {index + 1}</figcaption>
								</figure>
							{/each}
						</div>
					{:else}
						<div class="rendering-state">
							{#if rendering}<span class="spinner" aria-hidden="true"></span>{/if}
							<span>{rendering ? 'Preparing pages' : 'No visible page content'}</span>
						</div>
					{/if}
				</div>
				<div class="status-line" role="status" aria-live="polite">
					<span class:ready={phase === 'ready'}>{phase === 'ready' ? '●' : '○'}</span>
					{exporting ? exportProgress : status}
				</div>
			</div>

			<ExportPanel
				{exporting}
				{rendering}
				{pngScale}
				onScale={(nextScale) => (pngScale = nextScale)}
				onCurrentSvg={() => void downloadCurrentSvg()}
				onCurrentPng={() => void downloadCurrentPng()}
				onArchive={(kind) => void downloadArchive(kind)}
				onJson={() => void downloadJson()}
				onCancel={cancel}
			/>
		</div>
		{#if error}<p class="error workspace-error" role="alert">{error}</p>{/if}
	</section>
{/if}

<style>
	.intro {
		width: min(100% - 2rem, 25rem);
		margin: auto;
	}

	h1 {
		margin: 0;
		font-size: 1.3rem;
		font-weight: 550;
		letter-spacing: -0.025em;
		line-height: 1.15;
	}

	.lede {
		margin: 0.45rem 0 1.15rem;
		color: var(--site-muted);
		font-size: 0.875rem;
		line-height: 1.5;
	}

	.drop-zone {
		display: inline-flex;
		width: 100%;
		height: 2.15rem;
		align-items: center;
		justify-content: center;
		gap: 0.45rem;
		border: 0;
		border-radius: 0.25rem;
		background: var(--site-text);
		padding: 0 1rem;
		color: var(--site-bg);
		font-size: 0.75rem;
		font-weight: 550;
		text-align: center;
		cursor: pointer;
		transition: opacity 140ms ease, transform 140ms ease;
	}

	.drop-zone:hover,
	.drop-zone:focus-visible {
		opacity: 0.82;
	}

	.drop-overlay {
		position: fixed;
		inset: 0;
		z-index: 100;
		display: grid;
		place-items: center;
		background: color-mix(in srgb, var(--site-bg) 96%, transparent);
		pointer-events: none;
	}

	.drop-overlay::after {
		position: absolute;
		inset: 1rem;
		border: 1px dashed var(--site-muted);
		border-radius: 0.35rem;
		content: '';
	}

	.drop-overlay-copy {
		display: flex;
		align-items: center;
		flex-direction: column;
		gap: 0.35rem;
		text-align: center;
	}

	.drop-overlay-copy strong {
		font-size: 1rem;
		font-weight: 550;
		letter-spacing: -0.015em;
	}

	.drop-overlay-copy span {
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.65rem;
	}

	.drop-hint {
		margin: 0.5rem 0 0;
		color: var(--site-muted);
		font-size: 0.7rem;
		text-align: center;
	}

	.visually-hidden {
		position: absolute;
		width: 1px;
		height: 1px;
		margin: -1px;
		clip: rect(0, 0, 0, 0);
		overflow: hidden;
		white-space: nowrap;
	}

	.error {
		margin: 1rem 0 0;
		border-left: 2px solid var(--color-danger);
		padding: 0.65rem 0.85rem;
		background: color-mix(in srgb, var(--color-danger) 8%, transparent);
		color: var(--site-text);
		font-family: var(--font-mono);
		font-size: 0.72rem;
	}

	.cancel-link {
		border: 0;
		background: transparent;
		padding: 0.75rem 0;
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.68rem;
		text-decoration: underline;
		text-underline-offset: 3px;
		cursor: pointer;
	}

	.workspace {
		display: flex;
		width: 100%;
		height: calc(100svh - 2.5rem);
		min-width: 0;
		min-height: 0;
		flex-direction: column;
		overflow: hidden;
	}

	.work-grid {
		display: grid;
		min-height: 0;
		flex: 1;
		grid-template-columns: 188px minmax(360px, 1fr) 188px;
	}

	.preview-panel {
		min-width: 0;
		background: var(--site-bg);
	}

	.preview-panel {
		display: flex;
		min-height: 0;
		flex-direction: column;
		overflow: hidden;
		background: var(--site-canvas);
	}

	.canvas-wrap {
		display: block;
		min-height: 0;
		flex: 1;
		overflow: auto;
		padding: clamp(0.65rem, 1.2vw, 1rem);
		background: var(--site-canvas);
		overscroll-behavior: contain;
	}

	.page-stack {
		display: flex;
		min-height: 100%;
		margin: 0 auto;
		align-items: center;
		flex-direction: column;
		gap: 0.85rem;
	}

	figure {
		display: flex;
		width: 100%;
		margin: 0;
		align-items: center;
		flex-direction: column;
		gap: 0.25rem;
		scroll-margin-top: 0.65rem;
	}

	figure img {
		display: block;
		width: 100%;
		max-width: 100%;
		border: 1px solid color-mix(in srgb, black 15%, transparent);
		background: white;
		box-shadow: 0 8px 24px color-mix(in srgb, black 16%, transparent);
	}

	.page-stack.fit-page figure img {
		width: auto;
		max-height: calc(100svh - 11.5rem);
	}

	figcaption {
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.58rem;
	}

	figure.active figcaption {
		color: var(--site-text);
	}

	.page-placeholder {
		display: grid;
		width: min(100%, 48rem);
		min-height: 60vh;
		place-items: center;
		border: 1px solid var(--site-border);
		background: var(--site-surface);
	}

	.rendering-state {
		display: flex;
		min-height: 100%;
		align-items: center;
		justify-content: center;
		gap: 0.7rem;
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.66rem;
	}

	.spinner {
		width: 1rem;
		height: 1rem;
		border: 1px solid var(--site-border);
		border-top-color: #167bff;
		border-radius: 50%;
		animation: spin 800ms linear infinite;
	}

	@keyframes spin { to { transform: rotate(360deg); } }

	.status-line {
		min-height: 1.75rem;
		border-top: 1px solid var(--site-border);
		padding: 0.45rem 0.6rem;
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.59rem;
	}

	.status-line span {
		margin-right: 0.35rem;
	}

	.status-line span.ready {
		color: var(--color-success);
	}

	.workspace-error {
		margin-top: 1rem;
	}

	@media (max-width: 1050px) {
		.work-grid {
			grid-template-columns: 180px minmax(360px, 1fr);
		}

	}

	@media (max-width: 720px) {
		.workspace {
			height: auto;
			min-height: calc(100svh - 2.5rem);
			overflow: visible;
		}

		.work-grid {
			display: flex;
			flex-direction: column;
		}

		.preview-panel {
			order: -1;
		}

		.canvas-wrap {
			min-height: 420px;
		}

	}

</style>
