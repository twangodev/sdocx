import { ConverterClient, type ConverterClientPort } from '$converter/client';
import { assertAcceptedFile, isLargeFile } from '$converter/protocol';
import { toInspectionView } from '$converter/view-model';
import { errorMessage, type LibraryDocument, type PreparedDocument } from './model';
import { createThumbnail } from './thumbnail';

export type ImportOutcome =
	| { filename: string; status: 'imported' | 'duplicate'; document: LibraryDocument }
	| { filename: string; status: 'failed'; error: string; unsavedFile?: File }
	| { filename: string; status: 'skipped' };

export interface ImportProgress {
	completed: number;
	total: number;
	filename: string;
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

export class ImportProcessor {
	private client: ConverterClientPort | null = null;
	private cancelled = false;
	private running = false;
	private generation = 0;

	constructor(
		private readonly save: SaveDocument,
		private readonly createClient: () => ConverterClientPort = () => new ConverterClient(),
		private readonly thumbnail: (svg: string) => Promise<Blob> = createThumbnail
	) {}

	cancel(): void {
		this.cancelled = true;
		this.client?.cancel();
	}

	async run(files: File[], options: ImportOptions = {}): Promise<ImportOutcome[]> {
		if (this.running) throw new Error('An import is already running.');
		this.running = true;
		this.cancelled = false;
		const results: ImportOutcome[] = [];
		try {
			this.client = this.createClient();
			for (const file of files) {
				if (this.cancelled) break;
				options.onProgress?.({
					completed: results.length,
					total: files.length,
					filename: file.name
				});
				let prepared: PreparedDocument | undefined;
				let result: ImportOutcome;
				try {
					assertAcceptedFile(file);
					if (isLargeFile(file) && !options.approveLargeFile?.(file)) {
						result = { filename: file.name, status: 'skipped' };
					} else {
						const bytes = await file.arrayBuffer();
						const digest = await crypto.subtle.digest('SHA-256', bytes);
						const contentHash = Array.from(new Uint8Array(digest), (byte) =>
							byte.toString(16).padStart(2, '0')
						).join('');
						if (this.cancelled) break;
						const summary = await this.client.load(bytes, ++this.generation);
						if (this.cancelled) break;
						let thumbnail: Blob | null = null;
						if (summary.pageCount > 0) {
							try {
								thumbnail = await this.thumbnail(await this.client.renderPage(0, 'light'));
							} catch {
								/* Thumbnails are optional derived assets. */
							}
						}
						if (this.cancelled) break;
						prepared = {
							file,
							contentHash,
							pageCount: summary.pageCount,
							title: toInspectionView(summary.inspection).title || file.name,
							thumbnail
						};
						const saved = await this.save(prepared, options.collectionId);
						result = {
							filename: file.name,
							status: saved.duplicate ? 'duplicate' : 'imported',
							document: saved.document
						};
					}
				} catch (error) {
					if (this.cancelled) break;
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
				await this.client.dispose(++this.generation);
			}
			return results;
		} finally {
			this.client?.destroy();
			this.client = null;
			this.running = false;
		}
	}
}
