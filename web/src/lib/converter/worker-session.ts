import type {
	ColorMode,
	ConverterRequest,
	DocumentSummary,
	WorkerPhase
} from './protocol';
import { BrowserDocumentSession } from './wasm-adapter';

interface ActiveDocumentSession {
	summary(): DocumentSummary;
	inspection(): unknown;
	renderPage(pageIndex: number, colorMode: ColorMode): string;
	dispose(): void;
}

type SessionFactory = (bytes: ArrayBuffer) => Promise<ActiveDocumentSession>;
type ProgressListener = (generation: number, phase: WorkerPhase, message: string) => void;

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
			case 'inspect':
				return this.requireSession().inspection();
			case 'renderPage':
				this.progress(request.generation, 'rendering', `Rendering page ${request.pageIndex + 1}`);
				return this.requireSession().renderPage(request.pageIndex, request.colorMode);
			case 'exportJson':
				return JSON.stringify(this.requireSession().inspection(), null, 2);
		}
	}

	private async load(request: Extract<ConverterRequest, { type: 'load' }>): Promise<DocumentSummary> {
		if (request.generation < this.generation) throw supersededLoad();
		this.generation = request.generation;
		this.disposeCurrent();
		this.progress(request.generation, 'loading', 'Loading the browser renderer');
		let next: ActiveDocumentSession | undefined;
		try {
			this.progress(request.generation, 'parsing', 'Parsing document locally');
			next = await this.createSession(request.bytes);
			this.assertCurrent(request.generation);
			this.progress(request.generation, 'inspecting', 'Reading document structure');
			const summary = next.summary();
			this.assertCurrent(request.generation);
			this.session = next;
			next = undefined;
			this.progress(request.generation, 'ready', 'Document ready');
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

	private progress(generation: number, phase: WorkerPhase, message: string): void {
		this.onProgress(generation, phase, message);
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
