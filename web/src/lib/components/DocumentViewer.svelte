<script lang="ts">
	import type { WorkerPhase } from '$converter/protocol';
	import type { InspectionView } from '$converter/view-model';
	import type { DocumentZoomCamera } from '$lib/viewer/document-zoom-camera.svelte';
	import DocumentInfoPanel from './DocumentInfoPanel.svelte';
	import DocumentCanvas from './viewer/DocumentCanvas.svelte';
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
		<DocumentCanvas
			pages={model.document.previewUrls}
			pageIndex={model.view.pageIndex}
			{zoom}
			disabled={model.view.exporting || model.view.rendering}
			busy={model.view.rendering}
			{onPageChange}
		>
			{#snippet page(url, index)}
				{#if url}
					<img
						data-page-zoom-target
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
			{/snippet}
			{#snippet empty()}
				<div class="rendering-state motion-fade-in flex min-h-full items-center justify-center gap-3 font-mono text-[0.66rem] text-muted">
					{#if model.view.rendering}
						<span class="spinner size-4 animate-spin rounded-full border border-subtle border-t-accent" aria-hidden="true"></span>
					{/if}
					<span>{model.view.rendering ? 'Preparing pages' : 'No visible page content'}</span>
				</div>
			{/snippet}
		</DocumentCanvas>
		<ViewerStatus
			phase={model.status.phase}
			status={model.status.message}
			exporting={model.view.exporting}
			exportProgress={model.status.exportProgress}
		/>
	</div>
</div>
