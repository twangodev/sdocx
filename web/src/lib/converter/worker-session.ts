import type { DebugRequest } from '$lib/debugger/model';
import type {
	ColorMode,
	ConverterRequest,
	DocumentSummary,
	PdfRenderResult,
	SvgRenderResult,
	WorkerPhase,
	ConverterProgress
} from './protocol';
import { BrowserDocumentSession } from './wasm-adapter';
import { pendingProgress, progressLabel, type ProcessingProgress, type WorkListener } from './progress';

interface ActiveDocumentSession {
	summary(): DocumentSummary;
	inspection(): unknown;
	resolvePages(selection: string): number[];
	exportPdf(pageIndices: number[], colorMode: ColorMode, onProgress?: WorkListener): Promise<PdfRenderResult>;
	debug?(request: DebugRequest): unknown;
	renderPage(pageIndex: number, colorMode: ColorMode, onProgress?: WorkListener): SvgRenderResult;
	dispose(): void;
}

type SessionFactory = (bytes: ArrayBuffer, onProgress: WorkListener) => Promise<ActiveDocumentSession>;
type ProgressListener = (event: ConverterProgress) => void;

export class ConverterWorkerSession {
	private session: ActiveDocumentSession | undefined;
	private generation = 0;

	constructor(
		private readonly onProgress: ProgressListener,
		private readonly createSession: SessionFactory = BrowserDocumentSession.create
	) {}

	async handle(request: ConverterRequest): Promise<unknown> {
		if (request.type === 'load') return this.load(request);
		if (request.type === 'dispose') {
			if (request.generation < this.generation) return null;
			this.generation = request.generation;
			this.disposeCurrent();
			return null;
		}

		this.assertCurrent(request.generation);
		switch (request.type) {
			case 'debug': {
				const session = this.requireSession();
				if (!session.debug) throw new Error('Debugger unavailable');
				return session.debug(request.request);
			}
			case 'inspect':
				return this.requireSession().inspection();
			case 'renderPage':
				return this.requireSession().renderPage(request.pageIndex, request.colorMode, this.observer(request));
			case 'exportPdf': {
				const result = await this.requireSession().exportPdf(request.pageIndices, request.colorMode, this.observer(request));
				this.assertCurrent(request.generation);
				return result;
			}
			case 'resolvePages':
				return this.requireSession().resolvePages(request.selection);
			case 'exportJson':
				return JSON.stringify(this.requireSession().inspection(), null, 2);
		}
	}

	private async load(request: Extract<ConverterRequest, { type: 'load' }>): Promise<DocumentSummary> {
		if (request.generation < this.generation) throw supersededLoad();
		this.generation = request.generation;
		this.disposeCurrent();
		const observer = this.observer(request);
		observer(pendingProgress('loading'));
		let next: ActiveDocumentSession | undefined;
		try {
			next = await this.createSession(request.bytes, observer);
			this.assertCurrent(request.generation);
			observer(pendingProgress('inspecting'));
			const summary = next.summary();
			this.assertCurrent(request.generation);
			this.session = next;
			next = undefined;
			observer({ stage: 'ready', completed: 1, total: 1 });
			return summary;
		} catch (error) {
			try {
				next?.dispose();
			} catch {
				// Preserve the load error instead of masking it with cleanup.
			}
			throw error;
		}
	}

	private observer(request: Extract<ConverterRequest, { type: 'load' | 'renderPage' | 'exportPdf' }>): WorkListener {
		return (progress: ProcessingProgress) => {
			if (request.generation !== this.generation) return;
			const phase: WorkerPhase = request.type !== 'load' ? 'rendering'
				: progress.stage === 'loading' ? 'loading'
				: progress.stage === 'inspecting' ? 'inspecting'
				: progress.stage === 'ready' ? 'ready' : 'parsing';
			this.onProgress({ type: 'progress', id: request.id, generation: request.generation,
				operation: request.type, phase, message: progressLabel(progress), progress });
		};
	}

	private assertCurrent(generation: number): void {
		if (generation !== this.generation) throw supersededLoad();
	}

	private requireSession(): ActiveDocumentSession {
		if (!this.session) throw new Error('Load a document before requesting its contents.');
		return this.session;
	}

	private disposeCurrent(): void {
		const current = this.session;
		this.session = undefined;
		current?.dispose();
	}
}

function supersededLoad(): Error {
	return new Error('Document load was superseded.');
}
