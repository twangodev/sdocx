<script lang="ts">
	import { onMount } from 'svelte';
	import DocumentViewer from '$lib/components/DocumentViewer.svelte';
	import DocumentToolbar from '$lib/components/DocumentToolbar.svelte';
	import DropOverlay from '$lib/components/DropOverlay.svelte';
	import ErrorNotice from '$lib/components/ErrorNotice.svelte';
	import UploadSurface from '$lib/components/UploadSurface.svelte';
	import { DocumentSession } from '$converter/document-session.svelte';
	import { DocumentZoomCamera } from '$lib/viewer/document-zoom-camera.svelte';

	let picker = $state<HTMLInputElement>();
	let pageIndex = $state(0);
	let detailsOpen = $state(false);

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

	function fitPreviewPage(): void {
		const selectedPage = pageIndex;
		zoom.fitPage();
		requestAnimationFrame(() => selectPage(selectedPage));
	}

	function onFileInput(event: Event): void {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		if (file) void session.load(file);
		input.value = '';
	}
</script>

<svelte:head>
	<title>sdocx — local Samsung Notes converter</title>
	<meta
		name="description"
		content="Inspect and export Samsung Notes .sdocx documents entirely in your browser."
	/>
</svelte:head>

<DropOverlay hasDocument={session.hasDocument} onFile={(file) => void session.load(file)} />

<input
	bind:this={picker}
	class="sr-only"
	type="file"
	accept=".sdocx,application/zip"
	onchange={onFileInput}
/>

{#if !session.hasDocument}
	<UploadSurface
		parsing={session.parsing}
		status={session.status}
		error={session.error}
		onOpen={() => picker?.click()}
		onCancel={() => session.cancel()}
	/>
{:else if session.summary && session.activeFile}
	<section class="workspace motion-surface-in" aria-label="Document converter">
		<DocumentToolbar
			model={{
				document: {
					title: session.details?.title || session.activeFile.name,
					filename: session.activeFile.name,
					fileSize: session.activeFile.size,
					pageCount: session.summary.pageCount
				},
				viewer: {
					pageIndex,
					previewZoom: zoom.visibleZoom,
					fitPage: zoom.visiblePageFit,
					colorMode: session.colorMode,
					detailsOpen
				},
				activity: {
					exporting: session.exporting,
					rendering: session.rendering,
					pngScale: session.pngScale,
					exportProgress: session.exportProgress
				}
			}}
			actions={{
				onToggleDetails: () => (detailsOpen = !detailsOpen),
				onSelectPage: selectPage,
				onStepPage: stepPage,
				onSetZoom: zoom.setZoom,
				onStepZoom: zoom.stepZoom,
				onFitWidth: zoom.fitWidth,
				onFitPage: fitPreviewPage,
				onColorMode: (nextMode) => void session.setColorMode(nextMode),
				onScale: (nextScale) => session.setPngScale(nextScale),
				onCurrentSvg: () => void session.downloadCurrentSvg(pageIndex),
				onCurrentPng: () => void session.downloadCurrentPng(pageIndex),
				onArchive: (kind) => void session.downloadArchive(kind),
				onJson: () => void session.downloadJson(),
				onCancel: () => session.cancel(),
				onReplace: () => picker?.click(),
				onClose: () => void session.close()
			}}
		/>
		<DocumentViewer
			model={{
				document: {
					pageCount: session.summary.pageCount,
					details: session.details,
					previewUrls: session.previewUrls
				},
				view: {
					pageIndex,
					detailsOpen,
					rendering: session.rendering,
					exporting: session.exporting
				},
				status: {
					phase: session.phase,
					message: session.status,
					exportProgress: session.exportProgress
				}
			}}
			{zoom}
			onPageChange={(nextPage) => (pageIndex = nextPage)}
		/>
		{#if session.error}<ErrorNotice message={session.error} />{/if}
	</section>
{/if}

<style>
	.workspace {
		display: flex;
		width: 100%;
		height: calc(100svh - 2.5rem);
		min-width: 0;
		min-height: 0;
		flex-direction: column;
		overflow: hidden;
	}

	@media (max-width: 720px) {
		.workspace {
			height: auto;
			min-height: calc(100svh - 2.5rem);
			overflow: visible;
		}
	}
</style>
