<script lang="ts">
	import type { WorkerPhase } from '$converter/protocol';
	import type { InspectionView } from '$converter/view-model';
	import type { DocumentZoomCamera } from '$lib/viewer/document-zoom-camera.svelte';
	import { wheelZoom } from '$lib/viewer/wheel-zoom';
	import DocumentInfoPanel from './DocumentInfoPanel.svelte';
	import ViewerStatus from './ViewerStatus.svelte';

	interface DocumentViewerModel {
		document: {
			pageCount: number;
			details: InspectionView | null;
			previewUrls: string[];
		};
		view: {
			pageIndex: number;
			detailsOpen: boolean;
			rendering: boolean;
			exporting: boolean;
		};
		status: {
			phase: WorkerPhase | null;
			message: string;
			exportProgress: string;
		};
	}

	interface Props {
		model: DocumentViewerModel;
		zoom: DocumentZoomCamera;
		onPageChange: (pageIndex: number) => void;
	}

	let { model, zoom, onPageChange }: Props = $props();

	function updatePageFromScroll(event: Event): void {
		const scroller = event.currentTarget as HTMLDivElement;
		const anchor = scroller.getBoundingClientRect().top + scroller.clientHeight * 0.3;
		const pages = scroller.querySelectorAll<HTMLElement>('[data-page-index]');
		let nextPage = model.view.pageIndex;

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

		if (nextPage !== model.view.pageIndex) onPageChange(nextPage);
	}
</script>

<div class="viewer-body" class:details-open={model.view.detailsOpen}>
	<div class="details-shell" aria-hidden={!model.view.detailsOpen} inert={!model.view.detailsOpen}>
		<DocumentInfoPanel
			pageCount={model.document.pageCount}
			details={model.document.details}
			open={model.view.detailsOpen}
			class="h-full w-56"
		/>
	</div>

	<div class="preview-panel">
		<div
			bind:this={zoom.scroller}
			use:wheelZoom={{
				disabled: model.view.exporting || model.view.rendering,
				onZoom: zoom.updateGesture,
				onEnd: () => void zoom.finishGesture()
			}}
			class="canvas-wrap"
			aria-busy={model.view.rendering}
			onscroll={updatePageFromScroll}
		>
			{#if model.document.previewUrls.length}
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
					{#each model.document.previewUrls as url, index}
						<figure class:active={model.view.pageIndex === index} data-page-index={index}>
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
					{#if model.view.rendering}<span class="spinner" aria-hidden="true"></span>{/if}
					<span>{model.view.rendering ? 'Preparing pages' : 'No visible page content'}</span>
				</div>
			{/if}
		</div>
		<ViewerStatus
			phase={model.status.phase}
			status={model.status.message}
			exporting={model.view.exporting}
			exportProgress={model.status.exportProgress}
		/>
	</div>
</div>

<style>
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

	@media (max-width: 720px) {
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
