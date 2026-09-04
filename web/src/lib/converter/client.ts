import type {
	ColorMode,
	ConverterEvent,
	ConverterRequest,
	DocumentSummary,
	WorkerPhase
} from './protocol';

type Pending = {
	resolve: (value: unknown) => void;
	reject: (reason: Error) => void;
};

export type ProgressListener = (generation: number, phase: WorkerPhase, message: string) => void;
type RequestPayload = ConverterRequest extends infer Request
	? Request extends { id: number }
		? Omit<Request, 'id'>
		: never
	: never;

export interface ConverterClientPort {
	load(bytes: ArrayBuffer, generation: number): Promise<DocumentSummary>;
	inspect(): Promise<unknown>;
	renderPage(pageIndex: number, colorMode: ColorMode): Promise<string>;
	exportJson(): Promise<string>;
	dispose(generation: number): Promise<void>;
	cancel(): void;
	destroy(): void;
}

export class ConverterClient implements ConverterClientPort {
	private worker: Worker;
	private nextId = 1;
	private generation = 0;
	private readonly pending = new Map<number, Pending>();

	constructor(private readonly onProgress?: ProgressListener) {
		this.worker = this.createWorker();
	}

	async load(bytes: ArrayBuffer, generation: number): Promise<DocumentSummary> {
		this.generation = generation;
		return (await this.request({ type: 'load', generation, bytes }, [bytes])) as DocumentSummary;
	}

	async inspect(): Promise<unknown> {
		return this.request({ type: 'inspect', generation: this.generation });
	}

	async renderPage(pageIndex: number, colorMode: ColorMode): Promise<string> {
		return (await this.request({
			type: 'renderPage',
			generation: this.generation,
			pageIndex,
			colorMode
		})) as string;
	}

	async exportJson(): Promise<string> {
		return (await this.request({ type: 'exportJson', generation: this.generation })) as string;
	}

	async dispose(generation: number): Promise<void> {
		this.generation = generation;
		await this.request({ type: 'dispose', generation });
	}

	cancel(): void {
		this.worker.terminate();
		this.rejectPending(new Error('Processing cancelled.'));
		this.worker = this.createWorker();
	}

	destroy(): void {
		this.worker.terminate();
		this.rejectPending(new Error('Converter closed.'));
	}

	private createWorker(): Worker {
		const worker = new Worker(new URL('./converter.worker.ts', import.meta.url), { type: 'module' });
		worker.onmessage = (event: MessageEvent<ConverterEvent>) => this.handleMessage(event.data);
		worker.onerror = (event) => {
			this.rejectPending(new Error(event.message || 'The browser worker stopped unexpectedly.'));
		};
		return worker;
	}

	private request(
		request: RequestPayload,
		transfer: Transferable[] = []
	): Promise<unknown> {
		const id = this.nextId++;
		return new Promise((resolve, reject) => {
			this.pending.set(id, { resolve, reject });
			this.worker.postMessage({ ...request, id } as ConverterRequest, transfer);
		});
	}

	private handleMessage(event: ConverterEvent): void {
		if (event.type === 'progress') {
			this.onProgress?.(event.generation, event.phase, event.message);
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
