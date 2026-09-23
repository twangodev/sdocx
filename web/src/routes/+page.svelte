<script lang="ts">
	import { getContext, onMount } from 'svelte';
	import { WORKSPACE, type WorkspaceState } from '$lib/workspace';
	import Debugger from '$lib/debugger/Debugger.svelte';
	import type { DebugPreview } from '$lib/debugger/model';
	import DocumentViewer from '$lib/components/DocumentViewer.svelte';
	import DocumentToolbar from '$lib/components/DocumentToolbar.svelte';
	import DropOverlay from '$lib/components/DropOverlay.svelte';
	import ErrorNotice from '$lib/components/ErrorNotice.svelte';
	import UploadNotice from '$lib/components/UploadNotice.svelte';
	import UploadSurface from '$lib/components/UploadSurface.svelte';
	import { DocumentSession } from '$converter/document-session.svelte';
	import { DocumentZoomCamera } from '$lib/viewer/document-zoom-camera.svelte';

	let uploadNotice = $state<{ codes: string[]; failed: boolean } | null>(null);
	let uploadGeneration = 0;
	let picker = $state<HTMLInputElement>();
	let pageIndex = $state(0);
	let detailsOpen = $state(true);
	const workspace = getContext<WorkspaceState>(WORKSPACE);
	const debuggerOpen = $derived(workspace.debuggerOpen);
	let debugPreview = $state.raw<DebugPreview | null>(null);
	$effect(() => {
		workspace.hasDocument = session.hasDocument;
	});

	const zoom = new DocumentZoomCamera(() => pageIndex);
	const session = new DocumentSession({
		onResetView: () => {
			uploadNotice = null;
			workspace.debuggerOpen = false;
			debugPreview = null;
			zoom.reset();
			pageIndex = 0;
			detailsOpen = true;
		}
	});

	onMount(() => {
		const stop = session.start();
		return () => {
			stop();
			workspace.hasDocument = false;
			workspace.debuggerOpen = false;
			debugPreview = null;
		};
	});

	function selectPage(nextPage: number): void {
		zoom.scrollToPage(nextPage);
		pageIndex = nextPage;
	}

	function stepPage(direction: -1 | 1): void {
		if (!session.summary) return;
		selectPage(
			Math.min(
				session.summary.pageCount - 1,
				Math.max(0, pageIndex + direction)
			)
		);
	}

	function fitPreviewPage(): void {
		zoom.fitSelectedPage();
	}

	async function loadDocument(file: File): Promise<void> {
		const generation = ++uploadGeneration;
		uploadNotice = null;
		await session.load(file);
		if (generation !== uploadGeneration) return;
		if (session.error || (session.activeFile === file && session.summary)) {
			uploadNotice = {
				codes: session.activeFile === file ? session.details?.diagnostics.map(({ code }) => code) ?? [] : [],
				failed: Boolean(session.error)
			};
		}
	}

	function onFileInput(event: Event): void {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		if (file) void loadDocument(file);
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

<DropOverlay
	hasDocument={session.hasDocument}
	onFile={(file) => void loadDocument(file)}
/>

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
	<section
		class="motion-surface-in flex h-[calc(100svh-2.5rem)] min-h-0 w-full min-w-0 flex-col overflow-hidden max-[720px]:h-auto max-[720px]:min-h-[calc(100svh-2.5rem)] max-[720px]:overflow-visible"
		aria-label="Document converter"
		style:height={debuggerOpen ? 'calc(100svh - 2.5rem)' : undefined}
		style:overflow={debuggerOpen ? 'hidden' : undefined}
	>
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
		<div class="relative flex min-h-0 min-w-0 flex-1">
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
				debugPreview={debuggerOpen ? debugPreview : null}
				{session}
				onPageChange={(nextPage) => (pageIndex = nextPage)}
			/>
			{#if debuggerOpen}
				{#key session.activeFile}
					<Debugger
						{session}
						viewerPage={pageIndex}
						onPageChange={selectPage}
						onPreviewChange={(preview) => (debugPreview = preview)}
						onClose={() => {
							workspace.debuggerOpen = false;
							debugPreview = null;
						}}
					/>
				{/key}
			{/if}
		</div>
		{#if session.error}<ErrorNotice message={session.error} />{/if}
	</section>
{/if}

{#if uploadNotice}
	<UploadNotice
		{...uploadNotice}
		onDismiss={() => (uploadNotice = null)}
	/>
{/if}
