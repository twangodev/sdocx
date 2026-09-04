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
	type WorkerPhase
} from './protocol';
import { toInspectionView, type InspectionView } from './view-model';

export type PngScale = 1 | 2;
export type ArchiveKind = 'svg' | 'png' | 'everything';

interface DocumentSessionOptions {
	onResetView?: () => void;
	createClient?: (onProgress: ProgressListener) => ConverterClientPort;
}

export class DocumentSession {
	activeFile = $state<File | null>(null);
	summary = $state<DocumentSummary | null>(null);
	details = $state<InspectionView | null>(null);
	colorMode = $state<ColorMode>('auto');
	pngScale = $state<PngScale>(1);
	previewUrls = $state<string[]>([]);
	phase = $state<WorkerPhase | null>(null);
	status = $state('Waiting for a document');
	error = $state('');
	parsing = $state(false);
	rendering = $state(false);
	exporting = $state(false);
	exportProgress = $state('');

	private client: ConverterClientPort | undefined;
	private loadGeneration = 0;
	private renderGeneration = 0;
	private readonly onResetView?: () => void;
	private readonly createClient: (onProgress: ProgressListener) => ConverterClientPort;

	constructor(options: DocumentSessionOptions = {}) {
		this.onResetView = options.onResetView;
		this.createClient = options.createClient ?? ((onProgress) => new ConverterClient(onProgress));
	}

	get hasDocument(): boolean {
		return this.summary !== null && this.activeFile !== null;
	}

	get stem(): string {
		return this.activeFile ? sanitizeStem(this.activeFile.name) : 'document';
	}

	start(): () => void {
		this.client = this.createClient((generation, phase, message) => {
			if (generation !== this.loadGeneration) return;
			this.phase = phase;
			this.status = message;
		});
		return () => this.destroy();
	}

	destroy(): void {
		this.loadGeneration += 1;
		this.client?.destroy();
		this.client = undefined;
		this.releasePreviews();
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
			const bytes = await file.arrayBuffer();
			if (generation !== this.loadGeneration) return;
			const nextSummary = await this.requireClient().load(bytes, generation);
			if (generation !== this.loadGeneration) return;
			this.summary = nextSummary;
			this.details = toInspectionView(nextSummary.inspection);
			this.status = `${nextSummary.pageCount} ${nextSummary.pageCount === 1 ? 'page' : 'pages'} ready`;
			this.phase = 'ready';

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

	async close(): Promise<void> {
		const generation = ++this.loadGeneration;
		try {
			await this.requireClient().dispose(generation);
		} finally {
			if (generation !== this.loadGeneration) return;
			this.clearDocument();
			this.phase = null;
			this.status = 'Waiting for a document';
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

	setPngScale(scale: PngScale): void {
		this.pngScale = scale;
	}

	async downloadCurrentSvg(pageIndex: number): Promise<void> {
		await this.withExport(async () => {
			const svg = await this.requireClient().renderPage(pageIndex, this.colorMode);
			downloadBlob(
				new Blob([svg], { type: 'image/svg+xml' }),
				pageFilename(this.stem, pageIndex, 'svg')
			);
		});
	}

	async downloadCurrentPng(pageIndex: number): Promise<void> {
		await this.withExport(async () => {
			const svg = await this.requireClient().renderPage(pageIndex, this.colorMode);
			const png = await svgToPng(svg, this.pngScale);
			downloadBlob(png, pageFilename(this.stem, pageIndex, 'png'));
		});
	}

	async downloadJson(): Promise<void> {
		await this.withExport(async () => {
			const json = await this.requireClient().exportJson();
			downloadBlob(new Blob([json], { type: 'application/json' }), `${this.stem}.json`);
		});
	}

	async downloadArchive(kind: ArchiveKind): Promise<void> {
		if (!this.summary || !this.activeFile) return;
		await this.withExport(async () => {
			const pageCount = this.summary!.pageCount;
			const sourceName = this.activeFile!.name;
			const client = this.requireClient();
			const inspectionJson = kind === 'everything' ? await client.exportJson() : '';
			const manifest = createExportManifest(
				sourceName,
				pageCount,
				this.colorMode,
				this.pngScale
			);

			const entries = this.archiveEntries(kind, pageCount, inspectionJson, manifest);
			const archive = await createZip(entries);
			downloadBlob(archive, `${this.stem}-${kind}.zip`);
		});
	}

	private async renderPreviews(): Promise<void> {
		if (!this.summary || this.summary.pageCount === 0) return;
		const generation = ++this.renderGeneration;
		const pageCount = this.summary.pageCount;
		this.rendering = true;
		this.error = '';
		this.releasePreviews();
		this.previewUrls = Array(pageCount).fill('');
		try {
			for (let index = 0; index < pageCount; index += 1) {
				this.status = `Rendering page ${index + 1} of ${pageCount}`;
				const svg = await this.requireClient().renderPage(index, this.colorMode);
				if (generation !== this.renderGeneration) return;
				const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
				this.previewUrls[index] = url;
				this.previewUrls = [...this.previewUrls];
			}
			this.status = `${pageCount} ${pageCount === 1 ? 'page' : 'pages'} rendered locally`;
		} catch (cause) {
			if (generation === this.renderGeneration) this.error = messageFrom(cause);
		} finally {
			if (generation === this.renderGeneration) this.rendering = false;
		}
	}

	private async *archiveEntries(
		kind: ArchiveKind,
		pageCount: number,
		inspectionJson: string,
		manifest: ReturnType<typeof createExportManifest>
	): AsyncGenerator<{ name: string; bytes: Uint8Array }> {
		if (kind === 'everything') {
			yield { name: 'document.json', bytes: textBytes(inspectionJson) };
			yield { name: 'manifest.json', bytes: textBytes(JSON.stringify(manifest, null, 2)) };
		}

		for (let index = 0; index < pageCount; index += 1) {
			this.exportProgress = `Rendering page ${index + 1} of ${pageCount}`;
			const svg = await this.requireClient().renderPage(index, this.colorMode);
			if (kind === 'svg' || kind === 'everything') {
				yield { name: pageFilename(this.stem, index, 'svg'), bytes: textBytes(svg) };
			}
			if (kind === 'png' || kind === 'everything') {
				this.exportProgress = `Rasterizing page ${index + 1} of ${pageCount}`;
				const png = await svgToPng(svg, this.pngScale);
				yield {
					name: pageFilename(this.stem, index, 'png'),
					bytes: new Uint8Array(await png.arrayBuffer())
				};
			}
		}
	}

	private clearDocument(): void {
		this.renderGeneration += 1;
		this.releasePreviews();
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
		this.exporting = true;
		this.error = '';
		this.exportProgress = 'Preparing download';
		try {
			await task();
			this.status = 'Download ready';
		} catch (cause) {
			this.error = messageFrom(cause);
		} finally {
			this.exporting = false;
			this.exportProgress = '';
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
