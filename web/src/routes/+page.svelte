<script lang="ts">
	import { onMount } from 'svelte';
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
	let fitPage = $state(false);
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
		fitPage = false;
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

	function onDrop(event: DragEvent): void {
		event.preventDefault();
		dragging = false;
		const file = event.dataTransfer?.files[0];
		if (file) void loadFile(file);
	}

	function onDragOver(event: DragEvent): void {
		event.preventDefault();
		event.dataTransfer!.dropEffect = 'copy';
		dragging = true;
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

	function formatBytes(bytes: number): string {
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
		return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
	}
</script>

<svelte:head>
	<title>sdocx — local Samsung Notes converter</title>
	<meta
		name="description"
		content="Inspect and export Samsung Notes .sdocx documents entirely in your browser."
	/>
</svelte:head>

{#if !hasDocument}
	<section class="intro" aria-labelledby="intro-title">
		<h1 id="intro-title">Open a Samsung Notes file</h1>
		<p class="lede">
			Preview and export .sdocx documents. Files stay in this browser.
		</p>

		<button
			type="button"
			class:dragging
			class="drop-zone"
			onclick={() => picker?.click()}
			ondrop={onDrop}
			ondragover={onDragOver}
			ondragleave={() => (dragging = false)}
		>
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
		<div class="workspace-heading">
			<div>
				<h1>{details?.title || activeFile.name}</h1>
				<p>{activeFile.name} · {formatBytes(activeFile.size)} · {summary.pageCount} {summary.pageCount === 1 ? 'page' : 'pages'} · local</p>
			</div>
			<div class="heading-actions">
				<button class="control" type="button" onclick={() => picker?.click()}>replace</button>
				<button class="control" type="button" onclick={() => void closeDocument()}>close</button>
			</div>
		</div>

		<input
			bind:this={picker}
			class="visually-hidden"
			type="file"
			accept=".sdocx,application/zip"
			onchange={onFileInput}
		/>

		<div class="work-grid">
			<aside class="panel document-panel" aria-label="Document information">
				<div class="panel-title"><span class="mono-label">inspect</span></div>
				<dl>
					<div><dt>Pages</dt><dd>{summary.pageCount}</dd></div>
					<div><dt>Format</dt><dd>{details?.formatVersion ?? 'Unknown'}</dd></div>
					<div><dt>Page size</dt><dd>{details?.dimensions ?? 'Varies'}</dd></div>
					<div><dt>Media</dt><dd>{details?.mediaCount ?? 0}</dd></div>
					{#if details?.created}<div><dt>Created</dt><dd>{details.created}</dd></div>{/if}
					{#if details?.modified}<div><dt>Modified</dt><dd>{details.modified}</dd></div>{/if}
				</dl>

				<div class="diagnostics">
					<div class="panel-title"><span class="mono-label">diagnostics</span></div>
					{#if details?.diagnostics.length}
						<ul>
							{#each details.diagnostics as diagnostic}
								<li>
									<strong>{diagnostic.code}</strong>
									<span>{diagnostic.message}</span>
									{#if diagnostic.entry}<code>{diagnostic.entry}</code>{/if}
								</li>
							{/each}
						</ul>
					{:else}
						<p class="clean"><span>✓</span> No parser warnings</p>
					{/if}
				</div>
			</aside>

			<div class="preview-panel">
				<div class="preview-toolbar">
					<div class="viewer-controls page-controls" role="group" aria-label="Page navigation">
						<button
							type="button"
							aria-label="Previous page"
							disabled={exporting || rendering || pageIndex === 0}
							onclick={() => stepPage(-1)}>‹</button
						>
						<label>
							<span class="visually-hidden">Page</span>
							<select
								value={pageIndex}
								disabled={exporting || rendering}
								onchange={(event) => selectPage(Number(event.currentTarget.value))}
							>
								{#each Array(summary.pageCount) as _, index}
									<option value={index}>page {String(index + 1).padStart(2, '0')} / {String(summary.pageCount).padStart(2, '0')}</option>
								{/each}
							</select>
						</label>
						<button
							type="button"
							aria-label="Next page"
							disabled={exporting || rendering || pageIndex === summary.pageCount - 1}
							onclick={() => stepPage(1)}>›</button
						>
					</div>
					<div class="viewer-controls zoom-controls" role="group" aria-label="Preview zoom">
						<button
							type="button"
							aria-label="Zoom out"
							disabled={exporting || rendering || (!fitPage && previewZoom === zoomSteps[0])}
							onclick={() => stepZoom(-1)}>−</button
						>
						<span class="zoom-value" aria-live="polite">{fitPage ? 'fit' : `${previewZoom}%`}</span>
						<button
							type="button"
							aria-label="Zoom in"
							disabled={exporting || rendering || (!fitPage && previewZoom === zoomSteps.at(-1))}
							onclick={() => stepZoom(1)}>+</button
						>
						<button
							type="button"
							class="fit-control"
							class:active={!fitPage && previewZoom === 100}
							aria-label="Fit width"
							aria-pressed={!fitPage && previewZoom === 100}
							disabled={exporting || rendering}
							onclick={fitPreviewWidth}>width</button
						>
						<button
							type="button"
							class="fit-control"
							class:active={fitPage}
							aria-label="Fit page"
							aria-pressed={fitPage}
							disabled={exporting || rendering}
							onclick={fitPreviewPage}>page</button
						>
					</div>
					<div class="mode-switch" aria-label="Document color mode">
						{#each ['auto', 'light', 'dark'] as mode}
							<button
								type="button"
								disabled={exporting || rendering}
								class:active={colorMode === mode}
								onclick={() => void selectColorMode(mode as ColorMode)}
							>{mode}</button>
						{/each}
					</div>
				</div>
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

			<aside class="panel export-panel" aria-label="Export options">
				<div class="panel-title"><span class="mono-label">export</span></div>
				<div class="export-group">
					<span class="group-label">Current page</span>
					<button class="control primary" disabled={exporting || rendering} onclick={downloadCurrentSvg}>SVG <span>↓</span></button>
					<button class="control" disabled={exporting || rendering} onclick={downloadCurrentPng}>PNG <span>↓</span></button>
					<label class="scale-control">
						<span>PNG scale</span>
						<select bind:value={pngScale}><option value={1}>1×</option><option value={2}>2×</option></select>
					</label>
				</div>

				<div class="export-group">
					<span class="group-label">Whole document</span>
					<button class="control" disabled={exporting} onclick={() => void downloadArchive('svg')}>all SVG <span>.zip</span></button>
					<button class="control" disabled={exporting} onclick={() => void downloadArchive('png')}>all PNG <span>.zip</span></button>
					<button class="control" disabled={exporting} onclick={() => void downloadArchive('everything')}>everything <span>.zip</span></button>
				</div>

				<div class="export-group final">
					<span class="group-label">Structure</span>
					<button class="control" disabled={exporting} onclick={downloadJson}>document JSON <span>↓</span></button>
				</div>

				{#if exporting}
					<button class="cancel-link" type="button" onclick={cancel}>cancel export</button>
				{/if}
			</aside>
		</div>
		{#if error}<p class="error workspace-error" role="alert">{error}</p>{/if}
	</section>
{/if}

<style>
	.intro {
		width: min(100% - 3rem, 28rem);
		margin: auto;
	}

	h1 {
		margin: 0;
		font-size: 1.45rem;
		font-weight: 550;
		letter-spacing: -0.025em;
		line-height: 1.15;
	}

	.lede {
		margin: 0.65rem 0 1.75rem;
		color: var(--site-muted);
		font-size: 0.875rem;
		line-height: 1.5;
	}

	.drop-zone {
		display: grid;
		width: 100%;
		height: 2.4rem;
		align-items: center;
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
	.drop-zone.dragging {
		opacity: 0.82;
	}

	.drop-hint {
		margin: 0.7rem 0 0;
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
		height: calc(100svh - 3rem);
		min-width: 0;
		min-height: 0;
		flex-direction: column;
		overflow: hidden;
	}

	.workspace-heading {
		display: flex;
		min-height: 3.65rem;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		border-bottom: 1px solid var(--site-border);
		padding: 0.55rem 1rem;
	}

	.workspace-heading h1 {
		max-width: 60vw;
		overflow: hidden;
		font-size: 0.875rem;
		font-weight: 550;
		letter-spacing: 0;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.workspace-heading p {
		margin: 0.2rem 0 0;
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.6rem;
	}

	.heading-actions {
		display: flex;
		gap: 0.5rem;
	}

	.work-grid {
		display: grid;
		min-height: 0;
		flex: 1;
		grid-template-columns: 220px minmax(360px, 1fr) 220px;
	}

	.panel,
	.preview-panel {
		min-width: 0;
		background: var(--site-bg);
	}

	.panel {
		overflow: auto;
		padding: 0.9rem;
	}

	.document-panel {
		border-right: 1px solid var(--site-border);
	}

	.export-panel {
		border-left: 1px solid var(--site-border);
	}

	.panel-title {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		color: var(--site-muted);
	}

	dl {
		margin: 0.65rem 0 0;
	}

	dl div {
		display: grid;
		grid-template-columns: 0.7fr 1fr;
		gap: 0.5rem;
		border-bottom: 1px solid var(--site-border);
		padding: 0.55rem 0;
		font-size: 0.7rem;
	}

	dt {
		color: var(--site-muted);
	}

	dd {
		margin: 0;
		text-align: right;
		word-break: break-word;
	}

	.diagnostics {
		margin-top: 1.5rem;
	}

	.diagnostics ul {
		display: flex;
		margin: 0.8rem 0 0;
		padding: 0;
		flex-direction: column;
		gap: 0.65rem;
		list-style: none;
	}

	.diagnostics li {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		border-left: 2px solid #c9952f;
		padding-left: 0.65rem;
		font-size: 0.7rem;
	}

	.diagnostics li span,
	.diagnostics code {
		color: var(--site-muted);
	}

	.clean {
		margin: 0.9rem 0 0;
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.68rem;
	}

	.clean span {
		margin-right: 0.35rem;
		color: var(--color-success);
	}

	.preview-panel {
		display: flex;
		min-height: 0;
		flex-direction: column;
		overflow: hidden;
		background: var(--site-canvas);
	}

	.preview-toolbar {
		display: flex;
		min-height: 3rem;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		border-bottom: 1px solid var(--site-border);
		padding: 0.45rem 0.7rem;
	}

	.preview-toolbar select,
	.scale-control select {
		border: 0;
		background: transparent;
		color: var(--site-text);
		font-family: var(--font-mono);
		font-size: 0.65rem;
		cursor: pointer;
	}

	.viewer-controls {
		display: flex;
		height: 1.9rem;
		align-items: stretch;
		border: 1px solid var(--site-border);
		border-radius: 0.25rem;
		overflow: hidden;
	}

	.viewer-controls button {
		min-width: 1.8rem;
		border: 0;
		border-right: 1px solid var(--site-border);
		background: transparent;
		padding: 0 0.45rem;
		color: var(--site-muted);
		font-size: 0.75rem;
		cursor: pointer;
	}

	.viewer-controls button:last-child {
		border-right: 0;
	}

	.viewer-controls button:hover:not(:disabled),
	.viewer-controls button.active {
		background: var(--site-raised);
		color: var(--site-text);
	}

	.viewer-controls button:disabled {
		cursor: not-allowed;
		opacity: 0.35;
	}

	.page-controls label {
		display: flex;
		flex: 1;
		border-right: 1px solid var(--site-border);
	}

	.page-controls select {
		width: 100%;
		padding: 0 0.45rem;
	}

	.zoom-value {
		display: grid;
		min-width: 3rem;
		place-items: center;
		border-right: 1px solid var(--site-border);
		color: var(--site-text);
		font-family: var(--font-mono);
		font-size: 0.58rem;
	}

	.viewer-controls .fit-control {
		padding: 0 0.5rem;
		font-family: var(--font-mono);
		font-size: 0.55rem;
	}

	.mode-switch {
		display: flex;
		border: 1px solid var(--site-border);
		border-radius: 0.35rem;
		padding: 0.15rem;
	}

	.mode-switch button {
		border: 0;
		border-radius: 0.25rem;
		background: transparent;
		padding: 0.3rem 0.5rem;
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.58rem;
		cursor: pointer;
	}

	.mode-switch button.active {
		background: var(--site-raised);
		color: var(--site-text);
	}

	.canvas-wrap {
		display: block;
		min-height: 0;
		flex: 1;
		overflow: auto;
		padding: clamp(1rem, 2vw, 2rem);
		background: var(--site-canvas);
		overscroll-behavior: contain;
	}

	.page-stack {
		display: flex;
		min-height: 100%;
		margin: 0 auto;
		align-items: center;
		flex-direction: column;
		gap: 1.5rem;
	}

	figure {
		display: flex;
		width: 100%;
		margin: 0;
		align-items: center;
		flex-direction: column;
		gap: 0.45rem;
		scroll-margin-top: 1rem;
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
		max-height: calc(100svh - 17rem);
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
		min-height: 2.2rem;
		border-top: 1px solid var(--site-border);
		padding: 0.65rem 0.8rem;
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

	.export-group {
		display: flex;
		margin-top: 0.75rem;
		flex-direction: column;
		gap: 0.4rem;
		border-bottom: 1px solid var(--site-border);
		padding-bottom: 0.8rem;
	}

	.export-group.final {
		border-bottom: 0;
	}

	.group-label {
		margin-bottom: 0.2rem;
		color: var(--site-muted);
		font-size: 0.7rem;
	}

	.export-group .control {
		justify-content: space-between;
		width: 100%;
	}

	.export-group .control span {
		opacity: 0.65;
	}

	.scale-control {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.35rem 0.1rem;
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.62rem;
	}

	.workspace-error {
		margin-top: 1rem;
	}

	@media (max-width: 1050px) {
		.work-grid {
			grid-template-columns: 200px minmax(360px, 1fr);
		}

		.export-panel {
			grid-column: 1 / -1;
			border-top: 1px solid var(--site-border);
			border-left: 0;
		}

		.export-panel .export-group {
			display: grid;
			grid-template-columns: repeat(3, 1fr);
		}
	}

	@media (max-width: 720px) {
		.workspace {
			height: auto;
			min-height: calc(100svh - 3rem);
			overflow: visible;
		}

		.workspace-heading {
			align-items: start;
			flex-direction: column;
		}

		.work-grid {
			display: flex;
			flex-direction: column;
		}

		.document-panel,
		.export-panel {
			border: 0;
			border-top: 1px solid var(--site-border);
		}

		.preview-panel {
			order: -1;
		}

		.canvas-wrap {
			min-height: 420px;
		}

		.export-panel .export-group {
			display: flex;
		}
	}

	@media (max-width: 520px) {
		.preview-toolbar {
			align-items: stretch;
			flex-direction: column;
		}

		.mode-switch button {
			flex: 1;
		}
	}
</style>
