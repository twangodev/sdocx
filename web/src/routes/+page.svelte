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
	import LibraryWorkspace from '$lib/components/library/LibraryWorkspace.svelte';
	import { LibraryWorkspace as LibraryState } from '$lib/library/workspace.svelte';
	import { DocumentSession } from '$converter/document-session.svelte';
	import { DocumentZoomCamera } from '$lib/viewer/document-zoom-camera.svelte';

	let uploadNotice = $state<{ codes: string[]; failed: boolean } | null>(null);
	const library = new LibraryState();
	let activeLibraryDocument: { id: string; file: File } | null = null;
	let temporary = $state(false);
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
		onPageRendered: ({ file, pageIndex, svg }) => {
			if (pageIndex === 0 && activeLibraryDocument?.file === file) {
				void library.restoreThumbnail(activeLibraryDocument.id, svg);
			}
		},
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
		const stopLibrary = library.start();
		return () => {
			stopLibrary();
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

	async function exportResult(task: Promise<void>): Promise<string> {
		await task;
		return session.error;
	}

	async function openSaved(id: string): Promise<void> {
		await library.perform(async () => {
			const file = await library.service.openDocument(id);
			temporary = false;
			activeLibraryDocument = { id, file };
			await loadDocument(file);
		});
	}

	async function importFiles(files: File[]): Promise<void> {
		if (library.importing || session.exporting) return;
		await session.close();
		const results = await library.importFiles(files);
		if (files.length === 1 && !library.cancelled) {
			const result = results[0];
			if (result?.status === 'imported' || result?.status === 'duplicate') await openSaved(result.document.id);
		}
	}

	function openTemporary(file: File): void {
		activeLibraryDocument = null;
		temporary = true;
		void loadDocument(file);
	}

	function onFileInput(event: Event): void {
		const input = event.currentTarget as HTMLInputElement;
		const files = Array.from(input.files ?? []);
		if (files.length) void importFiles(files);
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
	onFiles={(files) => void importFiles(files)}
/>

<input
	bind:this={picker}
	class="sr-only"
	type="file"
	multiple
	accept=".sdocx,application/zip"
	onchange={onFileInput}
/>

{#if !session.hasDocument}
	<LibraryWorkspace {library} onImport={() => picker?.click()} onOpen={(id) => void openSaved(id)} onTemporary={openTemporary} />
	{#if session.parsing}<div role="status" class="fixed bottom-4 left-1/2 z-50 -translate-x-1/2 rounded border border-subtle bg-bg px-4 py-2 text-xs">{session.status}<button class="ml-4 underline" onclick={() => session.cancel()}>Cancel</button></div>{/if}
	{#if session.error}<div class="fixed bottom-4 left-4 z-50"><ErrorNotice message={session.error} /></div>{/if}
{:else if session.summary && session.activeFile}
	<section
		class="motion-surface-in flex h-[calc(100svh-2.5rem)] min-h-0 w-full min-w-0 flex-col overflow-hidden max-[720px]:h-auto max-[720px]:min-h-[calc(100svh-2.5rem)] max-[720px]:overflow-visible"
		aria-label="Document converter"
		style:height={debuggerOpen ? 'calc(100svh - 2.5rem)' : undefined}
		style:overflow={debuggerOpen ? 'hidden' : undefined}
	>
		{#if temporary}<p role="status" class="border-b border-subtle bg-surface px-3 py-2 text-xs">Temporary note · not saved to your library</p>{/if}
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
				onExport: (request) => exportResult(session.downloadExport(request)),
				onResolvePages: (selection) => session.resolvePages(selection),
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
			>
				{#snippet notification()}
					{#if uploadNotice}
						<UploadNotice {...uploadNotice} anchored onDismiss={() => (uploadNotice = null)} />
					{/if}
				{/snippet}
			</DocumentViewer>
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

{#if uploadNotice && !session.hasDocument}
	<UploadNotice
		{...uploadNotice}
		onDismiss={() => (uploadNotice = null)}
	/>
{/if}
