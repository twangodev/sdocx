<script lang="ts">
	import { onMount } from 'svelte';
	import { FolderOpen } from '@lucide/svelte';
	import DocumentInfoPanel from '$lib/components/DocumentInfoPanel.svelte';
	import DocumentToolbar from '$lib/components/DocumentToolbar.svelte';
	import { DocumentSession } from '$converter/document-session.svelte';
	import { DocumentZoomCamera } from '$lib/viewer/document-zoom-camera.svelte';
	import { wheelZoom } from '$lib/viewer/wheel-zoom';

	let picker = $state<HTMLInputElement>();
	let pageIndex = $state(0);
	let detailsOpen = $state(false);
	let dragging = $state(false);
	let dragDepth = 0;

	const zoom = new DocumentZoomCamera(() => pageIndex);
	const session = new DocumentSession({
		onResetView: () => {
			zoom.reset();
			pageIndex = 0;
			detailsOpen = false;
		}
	});

	onMount(() => session.start());

	function selectPage(nextPage: number): void {
		const page = zoom.scroller?.querySelector<HTMLElement>(`[data-page-index="${nextPage}"]`);
		page?.scrollIntoView({ behavior: 'auto', block: 'start' });
		pageIndex = nextPage;
	}

	function stepPage(direction: -1 | 1): void {
		if (!session.summary) return;
		selectPage(Math.min(session.summary.pageCount - 1, Math.max(0, pageIndex + direction)));
	}

	function alignSelectedPage(): void {
		const selectedPage = pageIndex;
		requestAnimationFrame(() => selectPage(selectedPage));
	}

	function fitPreviewPage(): void {
		zoom.fitPage();
		alignSelectedPage();
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
		if (file) void session.load(file);
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
		if (fileDrop && file) void session.load(file);
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
<div
	class="drop-overlay"
	class:visible={dragging}
	role="status"
	aria-live="polite"
	aria-hidden={!dragging}
>
	<div class="drop-overlay-copy">
		<strong>{session.hasDocument ? 'drop to replace document' : 'drop .sdocx to open'}</strong>
		<span>release anywhere · processed locally</span>
	</div>
</div>

{#if !session.hasDocument}
	<section class="intro motion-surface-in" aria-labelledby="intro-title">
		<h1 id="intro-title">Open a Samsung Notes file</h1>
		<p class="lede">
			Preview and export .sdocx documents. Files stay in this browser.
		</p>

		<button
			type="button"
			class="drop-zone"
			onclick={() => picker?.click()}
		>
			{#if !session.parsing}<FolderOpen size={13} strokeWidth={1.4} />{/if}
			{session.parsing ? session.status : 'open .sdocx'}
		</button>
		<p class="drop-hint">{session.parsing ? 'processing locally' : 'or drop one here · max 250 MiB'}</p>

		<input
			bind:this={picker}
			class="visually-hidden"
			type="file"
			accept=".sdocx,application/zip"
			onchange={onFileInput}
		/>
		{#if session.parsing}
			<button class="cancel-link" type="button" onclick={() => session.cancel()}>cancel parsing</button>
		{/if}
		{#if session.error}<p class="error motion-surface-in" role="alert">{session.error}</p>{/if}
	</section>
{:else if session.summary && session.activeFile}
	<section class="workspace motion-surface-in" aria-label="Document converter">
		<DocumentToolbar
			title={session.details?.title || session.activeFile.name}
			filename={session.activeFile.name}
			fileSize={session.activeFile.size}
			{pageIndex}
			pageCount={session.summary.pageCount}
			previewZoom={zoom.visibleZoom}
			fitPage={zoom.visiblePageFit}
			colorMode={session.colorMode}
			{detailsOpen}
			exporting={session.exporting}
			rendering={session.rendering}
			pngScale={session.pngScale}
			exportProgress={session.exportProgress}
			onToggleDetails={() => (detailsOpen = !detailsOpen)}
			onSelectPage={selectPage}
			onStepPage={stepPage}
			onSetZoom={zoom.setZoom}
			onStepZoom={zoom.stepZoom}
			onFitWidth={zoom.fitWidth}
			onFitPage={fitPreviewPage}
			onColorMode={(nextMode) => void session.setColorMode(nextMode)}
			onScale={(nextScale) => session.setPngScale(nextScale)}
			onCurrentSvg={() => void session.downloadCurrentSvg(pageIndex)}
			onCurrentPng={() => void session.downloadCurrentPng(pageIndex)}
			onArchive={(kind) => void session.downloadArchive(kind)}
			onJson={() => void session.downloadJson()}
			onCancel={() => session.cancel()}
			onReplace={() => picker?.click()}
			onClose={() => void session.close()}
		/>

		<input
			bind:this={picker}
			class="visually-hidden"
			type="file"
			accept=".sdocx,application/zip"
			onchange={onFileInput}
		/>

		<div class="viewer-body" class:details-open={detailsOpen}>
			<div class="details-shell" aria-hidden={!detailsOpen} inert={!detailsOpen}>
				<DocumentInfoPanel
					pageCount={session.summary.pageCount}
					details={session.details}
					open={detailsOpen}
					class="h-full w-56"
				/>
			</div>

			<div class="preview-panel">
				<div
					bind:this={zoom.scroller}
					use:wheelZoom={{
						disabled: !session.hasDocument || session.exporting || session.rendering,
						onZoom: zoom.updateGesture,
						onEnd: () => void zoom.finishGesture()
					}}
					class="canvas-wrap"
					aria-busy={session.rendering}
					onscroll={updatePageFromScroll}
				>
					{#if session.previewUrls.length}
						<div
							bind:this={zoom.surface}
							class="page-stack motion-page-in"
							class:fit-page={zoom.pageFit}
							class:zooming={zoom.gestureZoom !== null}
							class:recentering={zoom.recentering}
							data-zoom={zoom.visiblePageFit ? 'page' : zoom.visibleZoom}
							style:width={zoom.pageFit ? '100%' : `${zoom.committedZoom}%`}
							style:transform={zoom.surfaceTransform}
							style:transform-origin={`${zoom.gestureOrigin.x}px ${zoom.gestureOrigin.y}px`}
						>
							{#each session.previewUrls as url, index}
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
						<div class="rendering-state motion-fade-in">
							{#if session.rendering}<span class="spinner" aria-hidden="true"></span>{/if}
							<span>{session.rendering ? 'Preparing pages' : 'No visible page content'}</span>
						</div>
					{/if}
				</div>
				<div class="status-line" role="status" aria-live="polite">
					<span class:ready={session.phase === 'ready'}>{session.phase === 'ready' ? '●' : '○'}</span>
					{session.exporting ? session.exportProgress : session.status}
				</div>
			</div>
		</div>
		{#if session.error}<p class="error workspace-error motion-surface-in" role="alert">{session.error}</p>{/if}
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
		transition:
			opacity var(--motion-fast) var(--ease-standard),
			transform var(--motion-control) var(--ease-out);
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
		opacity: 0;
		pointer-events: none;
		visibility: hidden;
		transition:
			opacity var(--motion-standard) var(--ease-out),
			visibility 0s linear var(--motion-standard);
	}

	.drop-overlay.visible {
		opacity: 1;
		visibility: visible;
		transition-delay: 0s;
	}

	.drop-overlay::after {
		position: absolute;
		inset: 1rem;
		border: 1px dashed var(--site-muted);
		border-radius: 0.35rem;
		content: '';
		opacity: 0;
		transform: scale(0.992);
		transition:
			opacity var(--motion-standard) var(--ease-out),
			transform var(--motion-panel) var(--ease-out);
	}

	.drop-overlay.visible::after {
		opacity: 1;
		transform: scale(1);
	}

	.drop-overlay-copy {
		display: flex;
		align-items: center;
		flex-direction: column;
		gap: 0.35rem;
		text-align: center;
		opacity: 0;
		transform: translateY(6px) scale(0.985);
		transition:
			opacity var(--motion-standard) var(--ease-out),
			transform var(--motion-panel) var(--ease-out);
	}

	.drop-overlay.visible .drop-overlay-copy {
		opacity: 1;
		transform: translateY(0) scale(1);
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

	.viewer-body {
		position: relative;
		display: grid;
		grid-template-columns: 0 minmax(0, 1fr);
		min-height: 0;
		flex: 1;
		transition: grid-template-columns var(--motion-panel) var(--ease-out);
	}

	.viewer-body.details-open {
		grid-template-columns: 14rem minmax(0, 1fr);
	}

	.details-shell {
		min-width: 0;
		overflow: hidden;
		opacity: 0;
		pointer-events: none;
		transform: translateX(-8px);
		transition:
			opacity var(--motion-standard) var(--ease-standard),
			transform var(--motion-panel) var(--ease-out);
	}

	.viewer-body.details-open .details-shell {
		opacity: 1;
		pointer-events: auto;
		transform: translateX(0);
	}

	.preview-panel {
		display: flex;
		min-width: 0;
		min-height: 0;
		flex: 1;
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

	.motion-page-in {
		animation: page-stack-in var(--motion-standard) var(--ease-out) both;
	}

	.page-stack.zooming,
	.page-stack.recentering {
		will-change: transform;
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

	@keyframes page-stack-in {
		from { opacity: 0; }
		to { opacity: 1; }
	}

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

	@media (max-width: 720px) {
		.workspace {
			height: auto;
			min-height: calc(100svh - 2.5rem);
			overflow: visible;
		}

		.canvas-wrap {
			min-height: 420px;
		}

		.viewer-body {
			display: flex;
		}

		.details-shell {
			position: absolute;
			inset: 0 auto 0 0;
			z-index: 20;
			width: 14rem;
			background: var(--site-bg);
			box-shadow: 18px 0 40px color-mix(in srgb, black 18%, transparent);
			transform: translateX(-100%);
		}

		.viewer-body.details-open .details-shell {
			transform: translateX(0);
		}
	}

</style>
