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

<div
	class="viewer-body relative grid min-h-0 flex-1 transition-[grid-template-columns] duration-[var(--motion-panel)] ease-[var(--ease-out)] max-[720px]:flex {model
		.view.detailsOpen
		? 'grid-cols-[14rem_minmax(0,1fr)]'
		: 'grid-cols-[0_minmax(0,1fr)]'}"
	class:details-open={model.view.detailsOpen}
>
	<div
		class="details-shell min-w-0 overflow-hidden transition-[opacity,transform] duration-[var(--motion-panel)] ease-[var(--ease-out)] max-[720px]:absolute max-[720px]:inset-y-0 max-[720px]:left-0 max-[720px]:z-20 max-[720px]:w-56 max-[720px]:bg-bg max-[720px]:shadow-[18px_0_40px_rgb(0_0_0_/_0.18)] {model
			.view.detailsOpen
			? 'pointer-events-auto translate-x-0 opacity-100 max-[720px]:translate-x-0'
			: 'pointer-events-none -translate-x-2 opacity-0 max-[720px]:-translate-x-full'}"
		aria-hidden={!model.view.detailsOpen}
		inert={!model.view.detailsOpen}
	>
		<DocumentInfoPanel
			pageCount={model.document.pageCount}
			details={model.document.details}
			open={model.view.detailsOpen}
			class="h-full w-56"
		/>
	</div>

	<div class="preview-panel flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-canvas">
		<div
			bind:this={zoom.scroller}
			use:wheelZoom={{
				disabled: model.view.exporting || model.view.rendering,
				onZoom: zoom.updateGesture,
				onEnd: () => void zoom.finishGesture()
			}}
			class="canvas-wrap block min-h-0 flex-1 overflow-auto bg-canvas p-[clamp(0.65rem,1.2vw,1rem)] overscroll-contain max-[720px]:min-h-[420px]"
			aria-busy={model.view.rendering}
			onscroll={updatePageFromScroll}
		>
			{#if model.document.previewUrls.length}
				<div
					bind:this={zoom.surface}
					class="page-stack mx-auto my-0 flex min-h-full flex-col items-center gap-3.5 animate-[fade-in_var(--motion-standard)_var(--ease-out)_both] {zoom.gestureZoom !== null || zoom.recentering ? 'will-change-transform' : ''}"
					class:fit-page={zoom.pageFit}
					class:zooming={zoom.gestureZoom !== null}
					class:recentering={zoom.recentering}
					data-zoom={zoom.visiblePageFit ? 'page' : zoom.visibleZoom}
					style:width={zoom.pageFit ? '100%' : `${zoom.committedZoom}%`}
					style:transform={zoom.surfaceTransform}
					style:transform-origin={`${zoom.gestureOrigin.x}px ${zoom.gestureOrigin.y}px`}
				>
					{#each model.document.previewUrls as url, index}
						<figure
							class="m-0 flex w-full scroll-mt-2.5 flex-col items-center gap-1"
							class:active={model.view.pageIndex === index}
							data-page-index={index}
						>
							{#if url}
								<img
									class="block border border-black/15 bg-white shadow-[0_8px_24px_rgb(0_0_0_/_0.16)] {zoom.pageFit
										? 'h-auto w-auto max-h-[calc(100svh-11.5rem)] max-w-full'
										: 'w-full max-w-full'}"
									src={url}
									alt={`Rendered preview of page ${index + 1}`}
								/>
							{:else}
								<div class="page-placeholder grid min-h-[60vh] w-[min(100%,48rem)] place-items-center border border-subtle bg-surface">
									<span class="spinner size-4 animate-spin rounded-full border border-subtle border-t-accent" aria-hidden="true"></span>
								</div>
							{/if}
							<figcaption
								class="font-mono text-[0.58rem] {model.view.pageIndex === index ? 'text-text' : 'text-muted'}"
								>page {index + 1}</figcaption
							>
						</figure>
					{/each}
				</div>
			{:else}
				<div class="rendering-state motion-fade-in flex min-h-full items-center justify-center gap-3 font-mono text-[0.66rem] text-muted">
					{#if model.view.rendering}
						<span class="spinner size-4 animate-spin rounded-full border border-subtle border-t-accent" aria-hidden="true"></span>
					{/if}
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
