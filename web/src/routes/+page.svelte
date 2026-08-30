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
	let previewSvg = $state('');
	let previewUrl = $state('');
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

	onMount(() => {
		client = new ConverterClient((nextPhase, message) => {
			phase = nextPhase;
			status = message;
		});

		return () => {
			client.destroy();
			releasePreview();
		};
	});

	function releasePreview(): void {
		if (previewUrl) URL.revokeObjectURL(previewUrl);
		previewUrl = '';
	}

	function clearDocument(): void {
		renderGeneration += 1;
		releasePreview();
		activeFile = null;
		summary = null;
		details = null;
		pageIndex = 0;
		previewSvg = '';
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

			if (nextSummary.pageCount > 0) await renderPreview();
		} catch (cause) {
			clearDocument();
			error = messageFrom(cause);
			status = 'Could not open document';
		} finally {
			parsing = false;
		}
	}

	async function renderPreview(): Promise<void> {
		if (!summary || summary.pageCount === 0) return;
		const generation = ++renderGeneration;
		rendering = true;
		error = '';
		try {
			const svg = await client.renderPage(pageIndex, colorMode);
			if (generation !== renderGeneration) return;
			previewSvg = svg;
			releasePreview();
			previewUrl = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
			status = `Page ${pageIndex + 1} rendered locally`;
		} catch (cause) {
			if (generation === renderGeneration) error = messageFrom(cause);
		} finally {
			if (generation === renderGeneration) rendering = false;
		}
	}

	async function selectPage(nextPage: number): Promise<void> {
		pageIndex = nextPage;
		await renderPreview();
	}

	async function selectColorMode(nextMode: ColorMode): Promise<void> {
		colorMode = nextMode;
		await renderPreview();
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
			const svg = previewSvg || (await client.renderPage(pageIndex, colorMode));
			downloadBlob(new Blob([svg], { type: 'image/svg+xml' }), pageFilename(stem, pageIndex, 'svg'));
		});
	}

	async function downloadCurrentPng(): Promise<void> {
		await withExport(async () => {
			const svg = previewSvg || (await client.renderPage(pageIndex, colorMode));
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
		<div class="eyebrow mono-label">Local document tool / alpha</div>
		<h1 id="intro-title">Open a Samsung note.<br /><em>Keep it yours.</em></h1>
		<p class="lede">
			Inspect pages, diagnose compatibility, and export clean SVG, PNG, or JSON. Parsing and
			rendering happen inside this tab—your note is never uploaded.
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
			<span class="drop-icon" aria-hidden="true">↳</span>
			<span class="drop-copy">
				<strong>{parsing ? status : 'Drop an .sdocx here'}</strong>
				<small>{parsing ? 'Everything is happening locally' : 'or choose one from this device · max 250 MiB'}</small>
			</span>
			<span class="pick-label mono-label">{parsing ? 'working' : 'choose file'}</span>
		</button>

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
		<div class="trust-row" aria-label="Privacy guarantees">
			<span><b>01</b> no upload</span>
			<span><b>02</b> open source parser</span>
			<span><b>03</b> worker isolated</span>
		</div>
	</section>
{:else if summary && activeFile}
	<section class="workspace" aria-label="Document converter">
		<div class="workspace-heading">
			<div>
				<span class="mono-label">Current document</span>
				<h1>{details?.title || activeFile.name}</h1>
				<p>{activeFile.name} · {formatBytes(activeFile.size)} · processed locally</p>
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
				<div class="panel-title"><span class="mono-label">Inspect</span><i></i></div>
				<dl>
					<div><dt>Pages</dt><dd>{summary.pageCount}</dd></div>
					<div><dt>Format</dt><dd>{details?.formatVersion ?? 'Unknown'}</dd></div>
					<div><dt>Page size</dt><dd>{details?.dimensions ?? 'Varies'}</dd></div>
					<div><dt>Media</dt><dd>{details?.mediaCount ?? 0}</dd></div>
					{#if details?.created}<div><dt>Created</dt><dd>{details.created}</dd></div>{/if}
					{#if details?.modified}<div><dt>Modified</dt><dd>{details.modified}</dd></div>{/if}
				</dl>

				<div class="diagnostics">
					<div class="panel-title"><span class="mono-label">Diagnostics</span><i></i></div>
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
					<label>
						<span class="visually-hidden">Page</span>
						<select
							value={pageIndex}
							disabled={exporting || rendering}
							onchange={(event) => void selectPage(Number(event.currentTarget.value))}
						>
							{#each Array(summary.pageCount) as _, index}
								<option value={index}>page {String(index + 1).padStart(2, '0')} / {String(summary.pageCount).padStart(2, '0')}</option>
							{/each}
						</select>
					</label>
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
				<div class="canvas-wrap" aria-busy={rendering}>
					{#if previewUrl && !rendering}
						<img src={previewUrl} alt={`Rendered preview of page ${pageIndex + 1}`} />
					{:else}
						<div class="rendering-state">
							<span class="spinner" aria-hidden="true"></span>
							<span>{rendering ? `Rendering page ${pageIndex + 1}` : 'No visible page content'}</span>
						</div>
					{/if}
				</div>
				<div class="status-line" role="status" aria-live="polite">
					<span class:ready={phase === 'ready'}>{phase === 'ready' ? '●' : '○'}</span>
					{exporting ? exportProgress : status}
				</div>
			</div>

			<aside class="panel export-panel" aria-label="Export options">
				<div class="panel-title"><span class="mono-label">Export</span><i></i></div>
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
		width: min(100%, 980px);
		margin: auto;
	}

	.eyebrow {
		margin-bottom: 1.2rem;
		color: #167bff;
	}

	h1 {
		margin: 0;
		font-size: clamp(2.6rem, 7vw, 6.4rem);
		font-weight: 420;
		letter-spacing: -0.065em;
		line-height: 0.93;
	}

	h1 em {
		color: var(--site-muted);
		font-weight: 360;
	}

	.lede {
		max-width: 660px;
		margin: 2rem 0;
		color: var(--site-muted);
		font-size: clamp(1rem, 2vw, 1.2rem);
		line-height: 1.5;
	}

	.drop-zone {
		display: grid;
		width: 100%;
		min-height: 9rem;
		grid-template-columns: auto 1fr auto;
		align-items: center;
		gap: 1.25rem;
		border: 1px dashed var(--site-muted);
		border-radius: 0.6rem;
		background: color-mix(in srgb, var(--site-surface) 78%, transparent);
		padding: clamp(1.1rem, 4vw, 2rem);
		color: var(--site-text);
		text-align: left;
		cursor: pointer;
		transition: border-color 150ms ease, background 150ms ease;
	}

	.drop-zone:hover,
	.drop-zone.dragging {
		border-color: #167bff;
		background: color-mix(in srgb, #167bff 7%, var(--site-surface));
	}

	.drop-icon {
		display: grid;
		width: 3rem;
		height: 3rem;
		place-items: center;
		border: 1px solid var(--site-border);
		border-radius: 50%;
		font-family: var(--font-mono);
		font-size: 1.4rem;
	}

	.drop-copy {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.drop-copy strong {
		font-size: 1rem;
		font-weight: 520;
	}

	.drop-copy small {
		color: var(--site-muted);
		font-size: 0.8rem;
	}

	.pick-label {
		color: #167bff;
	}

	.trust-row {
		display: flex;
		margin-top: 1.4rem;
		flex-wrap: wrap;
		gap: 1rem 2rem;
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.62rem;
		text-transform: uppercase;
	}

	.trust-row b {
		margin-right: 0.35rem;
		color: var(--site-border);
		font-weight: 400;
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
		min-width: 0;
		flex-direction: column;
	}

	.workspace-heading {
		display: flex;
		align-items: end;
		justify-content: space-between;
		gap: 2rem;
		border-bottom: 1px solid var(--site-border);
		padding-bottom: 1.25rem;
	}

	.workspace-heading h1 {
		max-width: 800px;
		margin-top: 0.45rem;
		overflow: hidden;
		font-size: clamp(1.9rem, 4vw, 3.2rem);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.workspace-heading p {
		margin: 0.55rem 0 0;
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.65rem;
	}

	.heading-actions {
		display: flex;
		gap: 0.5rem;
	}

	.work-grid {
		display: grid;
		min-height: 650px;
		grid-template-columns: minmax(190px, 0.7fr) minmax(360px, 2.2fr) minmax(190px, 0.7fr);
		gap: clamp(1rem, 2vw, 2rem);
		padding-top: 1.5rem;
	}

	.panel,
	.preview-panel {
		min-width: 0;
		border: 1px solid var(--site-border);
		border-radius: 0.55rem;
		background: color-mix(in srgb, var(--site-bg) 90%, transparent);
	}

	.panel {
		padding: 1rem;
	}

	.panel-title {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		color: var(--site-muted);
	}

	.panel-title i {
		height: 1px;
		flex: 1;
		background: var(--site-border);
	}

	dl {
		margin: 1rem 0 0;
	}

	dl div {
		display: grid;
		grid-template-columns: 0.7fr 1fr;
		gap: 0.5rem;
		border-bottom: 1px solid var(--site-border);
		padding: 0.65rem 0;
		font-size: 0.75rem;
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
		margin-top: 2rem;
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
		background: var(--site-surface);
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
		display: grid;
		min-height: 540px;
		flex: 1;
		place-items: center;
		overflow: auto;
		padding: clamp(1rem, 3vw, 2.5rem);
		background:
			radial-gradient(circle, color-mix(in srgb, var(--site-muted) 23%, transparent) 0.7px, transparent 0.7px);
		background-size: 12px 12px;
	}

	.canvas-wrap img {
		display: block;
		max-width: 100%;
		max-height: 68vh;
		border: 1px solid color-mix(in srgb, black 15%, transparent);
		background: white;
		box-shadow: 0 18px 50px color-mix(in srgb, black 18%, transparent);
	}

	.rendering-state {
		display: flex;
		align-items: center;
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
		margin-top: 1rem;
		flex-direction: column;
		gap: 0.5rem;
		border-bottom: 1px solid var(--site-border);
		padding-bottom: 1rem;
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
			grid-template-columns: 1fr minmax(360px, 2fr);
		}

		.export-panel {
			grid-column: 1 / -1;
		}

		.export-panel .export-group {
			display: grid;
			grid-template-columns: repeat(3, 1fr);
		}
	}

	@media (max-width: 720px) {
		.workspace-heading {
			align-items: start;
			flex-direction: column;
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

		.export-panel .export-group {
			display: flex;
		}
	}

	@media (max-width: 520px) {
		.drop-zone {
			grid-template-columns: auto 1fr;
		}

		.pick-label {
			display: none;
		}

		.preview-toolbar {
			align-items: stretch;
			flex-direction: column;
		}

		.mode-switch button {
			flex: 1;
		}
	}
</style>
