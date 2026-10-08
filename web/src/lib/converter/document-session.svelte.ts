import type { DebugRequest } from '$lib/debugger/model';
import {
	ConverterClient,
	type ConverterClientPort,
	type ProgressListener
} from './client';
import {
	createExportManifest,
	createZip,
	downloadBlob,
	pageFilename,
	sanitizeStem,
	svgToPng,
	textBytes
} from './files';
import {
	assertAcceptedFile,
	isLargeFile,
	type ColorMode,
	type DocumentSummary,
	type PageRenderReport,
	type SvgRenderResult,
	type WorkerPhase
} from './protocol';
import { exportDetails, type ExportRequest } from './export-options';
import { toInspectionView, type InspectionView } from './view-model';
import { pageReport } from './render-reports';
import { pendingProgress, type ProcessingProgress, type WorkListener } from './progress';


export interface RenderedPage {
	file: File;
	pageIndex: number;
	svg: string;
}

interface DocumentSessionOptions {
	onPageRendered?: (page: RenderedPage) => void;
	onResetView?: () => void;
	createClient?: (onProgress: ProgressListener) => ConverterClientPort;
}

export class DocumentSession {
	activeFile = $state<File | null>(null);
	summary = $state<DocumentSummary | null>(null);
	details = $state<InspectionView | null>(null);
	colorMode = $state<ColorMode>('auto');
	previewUrls = $state<string[]>([]);
	previewReports = $state<PageRenderReport[]>([]);
	previewColorMode = $state<ColorMode>('auto');
	exportReports = $state<PageRenderReport[]>([]);
	exportColorMode = $state<ColorMode>('auto');
	phase = $state<WorkerPhase | null>(null);
	status = $state('Waiting for a document');
	error = $state('');
	parsing = $state(false);
	rendering = $state(false);
	exporting = $state(false);
	exportProgress = $state('');
	progress = $state<ProcessingProgress | null>(null);
	exportWork = $state<ProcessingProgress | null>(null);

	private client: ConverterClientPort | undefined;
	private loadGeneration = 0;
	private renderGeneration = 0;
	private readonly onResetView?: () => void;
	private readonly onPageRendered?: (page: RenderedPage) => void;
	private readonly createClient: (onProgress: ProgressListener) => ConverterClientPort;

	constructor(options: DocumentSessionOptions = {}) {
		this.onResetView = options.onResetView;
		this.onPageRendered = options.onPageRendered;
		this.createClient = options.createClient ?? ((onProgress) => new ConverterClient(onProgress));
	}

	get hasDocument(): boolean {
		return this.summary !== null && this.activeFile !== null;
	}

	get stem(): string {
		return this.activeFile ? sanitizeStem(this.activeFile.name) : 'document';
	}

	start(): () => void {
		this.client = this.createClient((event) => {
			if (event.generation !== this.loadGeneration || event.operation !== 'load') return;
			this.phase = event.phase;
			this.status = event.message;
			this.progress = event.progress;
		});
		return () => this.destroy();
	}

	destroy(): void {
		this.loadGeneration += 1;
		this.client?.destroy();
		this.client = undefined;
		this.releasePreviews();
		this.previewReports = [];
		this.exportReports = [];
	}

	async load(file: File): Promise<void> {
		this.error = '';
		try {
			assertAcceptedFile(file);
		} catch (cause) {
			this.error = messageFrom(cause);
			this.status = 'Could not open document';
			return;
		}

		if (
			isLargeFile(file) &&
			!window.confirm(
				'This file is over 100 MiB. Parsing may use substantial memory. Continue locally?'
			)
		) {
			return;
		}

		const generation = ++this.loadGeneration;
		try {
			this.clearDocument();
			this.activeFile = file;
			this.parsing = true;
			this.status = 'Reading file from this device';
			this.phase = 'loading';
			this.progress = pendingProgress('reading');
			const bytes = await file.arrayBuffer();
			if (generation !== this.loadGeneration) return;
			const nextSummary = await this.requireClient().load(bytes, generation);
			if (generation !== this.loadGeneration) return;
			this.summary = nextSummary;
			this.details = toInspectionView(nextSummary.inspection);
			this.status = `${nextSummary.pageCount} ${nextSummary.pageCount === 1 ? 'page' : 'pages'} ready`;
			this.phase = 'ready';
			this.progress = null;

			if (nextSummary.pageCount > 0) await this.renderPreviews();
		} catch (cause) {
			if (generation !== this.loadGeneration) return;
			this.clearDocument();
			this.error = messageFrom(cause);
			this.status = 'Could not open document';
		} finally {
			if (generation === this.loadGeneration) this.parsing = false;
		}
	}

	async debug(request: DebugRequest): Promise<unknown> {
		const generation = this.loadGeneration;
		const client = this.requireClient();
		if (!client.debug) throw new Error('Debugger unavailable');
		const result = await client.debug(request);
		if (generation !== this.loadGeneration) throw new Error('Document replaced');
		return result;
	}

	async close(): Promise<void> {
		const generation = ++this.loadGeneration;
		try {
			await this.requireClient().dispose(generation);
		} finally {
			if (generation === this.loadGeneration) {
				this.clearDocument();
				this.phase = null;
				this.status = 'Waiting for a document';
			}
		}
	}

	cancel(): void {
		this.loadGeneration += 1;
		this.client?.cancel();
		this.clearDocument();
		this.parsing = false;
		this.rendering = false;
		this.exporting = false;
		this.exportProgress = '';
		this.phase = null;
		this.status = 'Processing cancelled';
	}

	async setColorMode(colorMode: ColorMode): Promise<void> {
		this.colorMode = colorMode;
		await this.renderPreviews();
	}


	async renderThumbnailSvg(file: File): Promise<string> {
		if (this.activeFile !== file) throw new Error('Document replaced.');
		const generation = this.loadGeneration;
		const result = await this.requireClient().renderPage(0, 'auto');
		if (generation !== this.loadGeneration) throw new Error('Document replaced.');
		return result.svg;
	}


	async renderExportPreview(pageIndex: number, colorMode: ColorMode, onProgress?: WorkListener): Promise<SvgRenderResult> {
		if (!this.summary || !this.activeFile) throw new Error('No document loaded.');
		if (!Number.isInteger(pageIndex) || pageIndex < 0 || pageIndex >= this.summary.pageCount) {
			throw new Error('Select a valid preview page.');
		}
		const generation = this.loadGeneration;
		const client = this.requireClient();
		const result = await (onProgress ? client.renderPage(pageIndex, colorMode, event => {
			if (generation === this.loadGeneration) onProgress(event.progress);
		}) : client.renderPage(pageIndex, colorMode));
		if (generation !== this.loadGeneration) throw new Error('Document replaced.');
		return result;
	}


	async resolvePages(selection: string): Promise<number[]> {
		const generation = this.loadGeneration;
		const indices = await this.requireClient().resolvePages(selection);
		if (generation !== this.loadGeneration) throw new Error('Document replaced.');
		return indices;
	}

	async downloadExport(request: ExportRequest): Promise<void> {
		if (!this.summary || !this.activeFile || this.exporting) return;
		const generation = this.loadGeneration;
		const stem = this.stem;
		const sourceName = this.activeFile.name;
		const pageCount = this.summary.pageCount;
		const colorMode = request.colorMode ?? this.colorMode;
		const client = this.requireClient();
		const { format, pngScale } = request;
		const indices = format === 'json' || format === 'everything'
			? Array.from({ length: pageCount }, (_, index) => index)
			: [...request.pageIndices];
		const { filename } = exportDetails({ format, pageIndices: indices, pngScale }, pageCount, stem);
		const assertCurrent = () => {
			if (generation !== this.loadGeneration) throw new Error('Export cancelled.');
		};
		const onProgress: ProgressListener = (event) => {
			if (generation === this.loadGeneration && this.exporting) this.exportWork = event.progress;
		};
		const reports: PageRenderReport[] = [];
		this.exportReports = [];
		this.exportColorMode = colorMode;
		await this.withExport(async () => {
			if (!indices.length || indices.some((index) => !Number.isInteger(index) || index < 0 || index >= pageCount)) {
				throw new Error('Select valid pages to export.');
			}
			let blob: Blob;
			if (format === 'pdf') {
				this.exportProgress = 'Generating PDF';
				const result = await client.exportPdf(indices, colorMode, onProgress);
				assertCurrent();
				reports.push(...result.pages);
				blob = new Blob([result.bytes], { type: 'application/pdf' });
			} else if (format === 'json') {
				this.exportWork = pendingProgress('inspecting');
				blob = new Blob([await client.exportJson()], { type: 'application/json' });
			} else if (format !== 'everything' && indices.length === 1) {
				const result = await client.renderPage(indices[0], colorMode, onProgress);
				assertCurrent();
				reports.push(pageReport(result));
				if (format === 'png') this.exportWork = pendingProgress('encoding');
				blob = format === 'png' ? await svgToPng(result.svg, pngScale) : new Blob([result.svg], { type: 'image/svg+xml' });
			} else {
				const session = this;
				async function* entries() {
					if (format === 'everything') {
						const json = await client.exportJson();
						assertCurrent();
						yield { name: 'document.json', bytes: textBytes(json) };
						const manifest = createExportManifest(sourceName, pageCount, colorMode, pngScale);
						yield { name: 'manifest.json', bytes: textBytes(JSON.stringify(manifest, null, 2)) };
					}
					for (const [position, index] of indices.entries()) {
						assertCurrent();
						session.exportProgress = `Rendering page ${position + 1} of ${indices.length}`;
						const result = await client.renderPage(index, colorMode, onProgress);
						assertCurrent();
						reports.push(pageReport(result));
						if (format === 'svg' || format === 'everything') {
							yield { name: pageFilename(stem, index, 'svg'), bytes: textBytes(result.svg) };
						}
						if (format === 'png' || format === 'everything') {
							session.exportWork = pendingProgress('encoding');
							session.exportProgress = `Rasterizing page ${position + 1} of ${indices.length}`;
							const png = await svgToPng(result.svg, pngScale);
							assertCurrent();
							yield { name: pageFilename(stem, index, 'png'), bytes: new Uint8Array(await png.arrayBuffer()) };
						}
						session.exportWork = pendingProgress('packaging');
					}
				}
				blob = await createZip(entries());
			}
			assertCurrent();
			downloadBlob(blob, filename);
			this.exportReports = reports;
		});
	}

	private async renderPreviews(): Promise<void> {
		if (!this.summary || this.summary.pageCount === 0) return;
		const generation = ++this.renderGeneration;
		const loadGeneration = this.loadGeneration;
		const pageCount = this.summary.pageCount;
		const colorMode = this.colorMode;
		const file = this.activeFile;
		this.rendering = true;
		this.phase = 'rendering';
		this.progress = pendingProgress('preparing');
		this.error = '';
		this.releasePreviews();
		this.previewUrls = Array(pageCount).fill('');
		this.previewReports = [];
		this.previewColorMode = colorMode;
		try {
			for (let index = 0; index < pageCount; index += 1) {
				this.status = `Rendering page ${index + 1} of ${pageCount}`;
				const result = await this.requireClient().renderPage(index, colorMode, (event) => {
					if (generation === this.renderGeneration && loadGeneration === this.loadGeneration) this.progress = event.progress;
				});
				if (generation !== this.renderGeneration || loadGeneration !== this.loadGeneration) return;
				this.previewReports = [...this.previewReports, pageReport(result)];
				if (file) this.onPageRendered?.({ file, pageIndex: index, svg: result.svg });
				const url = URL.createObjectURL(new Blob([result.svg], { type: 'image/svg+xml' }));
				this.previewUrls[index] = url;
				this.previewUrls = [...this.previewUrls];
			}
			this.status = `${pageCount} ${pageCount === 1 ? 'page' : 'pages'} rendered locally`;
		} catch (cause) {
			if (generation === this.renderGeneration) this.error = messageFrom(cause);
		} finally {
			if (generation === this.renderGeneration) {
				this.rendering = false;
				this.phase = this.error ? null : 'ready';
				this.progress = null;
			}
		}
	}

	private clearDocument(): void {
		this.progress = null;
		this.exportWork = null;
		this.exporting = false;
		this.exportProgress = '';
		this.renderGeneration += 1;
		this.releasePreviews();
		this.previewReports = [];
		this.exportReports = [];
		this.activeFile = null;
		this.summary = null;
		this.details = null;
		this.onResetView?.();
	}

	private releasePreviews(): void {
		for (const url of this.previewUrls) {
			if (url) URL.revokeObjectURL(url);
		}
		this.previewUrls = [];
	}

	private async withExport(task: () => Promise<void>): Promise<void> {
		const generation = this.loadGeneration;
		this.exporting = true;
		this.error = '';
		this.exportProgress = 'Preparing download';
		this.exportWork = pendingProgress('preparing');
		try {
			await task();
			if (generation === this.loadGeneration) this.status = 'Download started';
		} catch (cause) {
			if (generation === this.loadGeneration) this.error = messageFrom(cause);
		} finally {
			if (generation === this.loadGeneration) {
				this.exporting = false;
				this.exportProgress = '';
				this.exportWork = null;
			}
		}
	}

	private requireClient(): ConverterClientPort {
		if (!this.client) throw new Error('The converter is not ready yet.');
		return this.client;
	}
}

function messageFrom(cause: unknown): string {
	return cause instanceof Error ? cause.message : 'The document could not be processed.';
}
