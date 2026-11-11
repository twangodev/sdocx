import { ConverterClient, type ConverterClientPort, type ProgressListener } from '$converter/client';
import { assertAcceptedFile, isLargeFile } from '$converter/protocol';
import { toInspectionView } from '$converter/view-model';
import { errorMessage, type LibraryDocument, type PreparedDocument } from './model';
import { createThumbnail } from './thumbnail';
import { pendingProgress, type ProcessingProgress } from '$converter/progress';

export type ImportOutcome =
	| { filename: string; status: 'imported' | 'duplicate'; document: LibraryDocument }
	| { filename: string; status: 'failed'; error: string; unsavedFile?: File }
	| { filename: string; status: 'skipped' };

export interface ImportProgress {
	completed: number;
	total: number;
	filename: string;
	work?: ProcessingProgress;
}

interface ImportOptions {
	collectionId?: string;
	onProgress?: (progress: ImportProgress) => void;
	onResult?: (result: ImportOutcome) => void;
	approveLargeFile?: (file: File) => boolean;
}

type SaveDocument = (
	document: PreparedDocument,
	collectionId?: string
) => Promise<{ document: LibraryDocument; duplicate: boolean }>;

interface ImportRun {
	client?: ConverterClientPort;
	cancelled: boolean;
}

export class ImportProcessor {
	private activeRun: ImportRun | undefined;
	private generation = 0;

	constructor(
		private readonly save: SaveDocument,
		private readonly createClient: (onProgress: ProgressListener) => ConverterClientPort = (onProgress) => new ConverterClient(onProgress),
		private readonly thumbnail: (svg: string) => Promise<Blob> = createThumbnail
	) {}

	cancel(): void {
		const run = this.activeRun;
		if (!run) return;
		run.cancelled = true;
		this.destroyClient(run);
	}

	async run(files: File[], options: ImportOptions = {}): Promise<ImportOutcome[]> {
		if (this.activeRun) throw new Error('An import is already running.');
		const run: ImportRun = { cancelled: false };
		this.activeRun = run;
		const results: ImportOutcome[] = [];
		try {
			let current: ImportProgress | undefined;
			const update = (work: ProcessingProgress) => {
				if (this.activeRun !== run || run.cancelled || !current) return;
				current = { ...current, work };
				options.onProgress?.(current);
			};
			const client = this.createClient((event) => {
				if (event.generation === this.generation) update(event.progress);
			});
			run.client = client;
			for (const file of files) {
				if (run.cancelled) break;
				current = { completed: results.length, total: files.length, filename: file.name };
				update(pendingProgress('reading'));
				if (run.cancelled) break;
				let prepared: PreparedDocument | undefined;
				let result: ImportOutcome;
				try {
					assertAcceptedFile(file);
					if (isLargeFile(file) && !options.approveLargeFile?.(file)) {
						result = { filename: file.name, status: 'skipped' };
					} else {
						const bytes = await file.arrayBuffer();
						if (run.cancelled) break;
						update(pendingProgress('hashing'));
						if (run.cancelled) break;
						const digest = await crypto.subtle.digest('SHA-256', bytes);
						const contentHash = Array.from(new Uint8Array(digest), (byte) =>
							byte.toString(16).padStart(2, '0')
						).join('');
						if (run.cancelled) break;
						const summary = await client.load(bytes, ++this.generation);
						if (run.cancelled) break;
						let thumbnail: Blob | null = null;
						if (summary.pageCount > 0) {
							try {
								const result = await client.renderPage(0, 'auto');
								if (run.cancelled) break;
								update(pendingProgress('encoding'));
								if (run.cancelled) break;
								thumbnail = await this.thumbnail(result.svg);
							} catch {
								/* Thumbnails are optional derived assets. */
							}
						}
						if (run.cancelled) break;
						prepared = {
							file,
							contentHash,
							pageCount: summary.pageCount,
							title: toInspectionView(summary.inspection).title || file.name,
							thumbnail
						};
						update(pendingProgress('saving'));
						if (run.cancelled) break;
						const saved = await this.save(prepared, options.collectionId);
						result = {
							filename: file.name,
							status: saved.duplicate ? 'duplicate' : 'imported',
							document: saved.document
						};
					}
				} catch (error) {
					if (run.cancelled) break;
					result = {
						filename: file.name,
						status: 'failed',
						error: errorMessage(error),
						unsavedFile: prepared?.file
					};
				}
				results.push(result);
				options.onResult?.(result);
				options.onProgress?.({
					completed: results.length,
					total: files.length,
					filename: file.name
				});
				current = undefined;
				if (run.cancelled) break;
				try {
					await client.dispose(++this.generation);
				} catch (error) {
					if (!run.cancelled) throw error;
				}
			}
			return results;
		} finally {
			try {
				this.destroyClient(run);
			} finally {
				this.activeRun = undefined;
			}
		}
	}

	private destroyClient(run: ImportRun): void {
		const client = run.client;
		run.client = undefined;
		client?.destroy();
	}
}
