import type { DebugRequest } from '$lib/debugger/model';
import type {
	ColorMode,
	ConverterEvent,
	ConverterProgress,
	ConverterRequestPayload,
	ConverterResponseMap,
	DocumentSummary,
	PdfRenderResult,
	SvgRenderResult,
} from './protocol';

type Pending = {
	resolve: (value: unknown) => void;
	reject: (reason: Error) => void;
	onProgress?: ProgressListener;
};

type StoppedWorkerState = { kind: 'failed' | 'closed'; error: Error };
type WorkerState = { kind: 'running'; worker: Worker } | StoppedWorkerState;

export type ProgressListener = (event: ConverterProgress) => void;

export interface ConverterClientPort {
	load(bytes: ArrayBuffer, generation: number): Promise<DocumentSummary>;
	inspect(): Promise<unknown>;
	debug?(request: DebugRequest): Promise<unknown>;
	renderPage(pageIndex: number, colorMode: ColorMode, onProgress?: ProgressListener): Promise<SvgRenderResult>;
	exportPdf(pageIndices: number[], colorMode: ColorMode, onProgress?: ProgressListener): Promise<PdfRenderResult>;
	resolvePages(selection: string): Promise<number[]>;
	exportJson(): Promise<string>;
	dispose(generation: number): Promise<void>;
	cancel(): void;
	destroy(): void;
}

export class ConverterClient implements ConverterClientPort {
	private state: WorkerState;
	private nextId = 1;
	private generation = 0;
	private readonly pending = new Map<number, Pending>();

	constructor(private readonly onProgress?: ProgressListener) {
		this.state = { kind: 'running', worker: this.createWorker() };
	}

	async load(bytes: ArrayBuffer, generation: number): Promise<DocumentSummary> {
		this.generation = generation;
		return this.request({ type: 'load', generation, bytes }, [bytes]);
	}

	async debug(request: DebugRequest): Promise<unknown> {
		return this.request({ type: 'debug', generation: this.generation, request });
	}

	async inspect(): Promise<unknown> {
		return this.request({ type: 'inspect', generation: this.generation });
	}

	async renderPage(pageIndex: number, colorMode: ColorMode, onProgress?: ProgressListener): Promise<SvgRenderResult> {
		return this.request({
			type: 'renderPage',
			generation: this.generation,
			pageIndex,
			colorMode
		}, [], onProgress);
	}

	async exportPdf(pageIndices: number[], colorMode: ColorMode, onProgress?: ProgressListener): Promise<PdfRenderResult> {
		return this.request({ type: 'exportPdf', generation: this.generation, pageIndices, colorMode }, [], onProgress);
	}

	async resolvePages(selection: string): Promise<number[]> {
		return this.request({ type: 'resolvePages', generation: this.generation, selection });
	}

	async exportJson(): Promise<string> {
		return this.request({ type: 'exportJson', generation: this.generation });
	}

	async dispose(generation: number): Promise<void> {
		this.generation = generation;
		await this.request({ type: 'dispose', generation });
	}

	cancel(): void {
		if (this.state.kind === 'closed') return;
		this.stopWorker({ kind: 'failed', error: new Error('Processing cancelled.') });
		this.state = { kind: 'running', worker: this.createWorker() };
	}

	destroy(): void {
		if (this.state.kind === 'closed') return;
		this.stopWorker({ kind: 'closed', error: new Error('Converter closed.') });
	}

	private createWorker(): Worker {
		const worker = new Worker(new URL('./converter.worker.ts', import.meta.url), { type: 'module' });
		worker.onmessage = (event: MessageEvent<ConverterEvent>) => {
			const state = this.state;
			if (state.kind === 'running' && worker === state.worker) this.handleMessage(event.data);
		};
		worker.onerror = (event) => {
			const state = this.state;
			if (state.kind !== 'running' || worker !== state.worker) return;
			this.stopWorker({
				kind: 'failed',
				error: new Error(event.message || 'The browser worker stopped unexpectedly.')
			});
		};
		return worker;
	}

	private stopWorker(next: StoppedWorkerState): void {
		const previous = this.state;
		this.state = next;
		if (previous.kind === 'running') {
			const worker = previous.worker;
			worker.onmessage = null;
			worker.onerror = null;
			worker.terminate();
		}
		this.rejectPending(next.error);
	}

	private request<Request extends ConverterRequestPayload>(
		request: Request,
		transfer: Transferable[] = [],
		onProgress?: ProgressListener
	): Promise<ConverterResponseMap[Request['type']]> {
		const state = this.state;
		if (state.kind !== 'running') return Promise.reject(state.error);
		const worker = state.worker;
		const id = this.nextId++;
		return new Promise((resolve, reject) => {
			this.pending.set(id, {
				resolve: (value) => resolve(value as ConverterResponseMap[Request['type']]),
				reject,
				onProgress
			});
			try {
				worker.postMessage({ ...request, id }, transfer);
			} catch (cause) {
				this.pending.delete(id);
				reject(cause);
			}
		});
	}

	private handleMessage(event: ConverterEvent): void {
		if (event.type === 'progress') {
			if (event.generation !== this.generation) return;
			const pending = this.pending.get(event.id);
			if (!pending) return;
			(pending.onProgress ?? this.onProgress)?.(event);
			return;
		}

		const pending = this.pending.get(event.id);
		if (!pending) return;
		this.pending.delete(event.id);

		if (event.type === 'error') pending.reject(new Error(event.message));
		else pending.resolve(event.value);
	}

	private rejectPending(error: Error): void {
		for (const pending of this.pending.values()) pending.reject(error);
		this.pending.clear();
	}
}
