import type { ColorMode } from '$converter/protocol';
import type {
	RenderedDocument,
	RendererArtifact,
	RendererRequest,
	RendererResponse
} from './commit-protocol';
import { rendererUrls } from './renderer-catalog';

interface Pending {
	resolve: (value: RenderedDocument | null) => void;
	reject: (error: Error) => void;
}

type RendererRequestPayload = RendererRequest extends infer Request
	? Request extends { id: number }
		? Omit<Request, 'id'>
		: never
	: never;

export class VersionedRenderer {
	private readonly worker = new Worker(new URL('./versioned-renderer.worker.ts', import.meta.url), {
		type: 'module'
	});
	private readonly pending = new Map<number, Pending>();
	private nextId = 1;

	constructor() {
		this.worker.onmessage = (event: MessageEvent<RendererResponse>) => this.receive(event.data);
		this.worker.onerror = (event) => this.rejectAll(event.message || 'The renderer worker stopped.');
	}

	async initialize(artifact: RendererArtifact, origin = location.origin): Promise<void> {
		const urls = rendererUrls(artifact, origin);
		await this.request({ type: 'initialize', ...urls });
	}

	async render(bytes: Uint8Array, colorMode: ColorMode = 'light'): Promise<RenderedDocument> {
		const buffer = Uint8Array.from(bytes).buffer;
		const result = await this.request({ type: 'render', bytes: buffer, colorMode }, [buffer]);
		if (!result) throw new Error('The renderer returned no document.');
		return result;
	}

	destroy(): void {
		this.worker.terminate();
		this.rejectAll('The renderer was closed.');
	}

	private request(
		request: RendererRequestPayload,
		transfer: Transferable[] = []
	): Promise<RenderedDocument | null> {
		const id = this.nextId++;
		return new Promise((resolve, reject) => {
			this.pending.set(id, { resolve, reject });
			this.worker.postMessage({ ...request, id } as RendererRequest, transfer);
		});
	}

	private receive(response: RendererResponse): void {
		const pending = this.pending.get(response.id);
		if (!pending) return;
		this.pending.delete(response.id);
		if (response.type === 'error') pending.reject(new Error(response.message));
		else pending.resolve(response.value);
	}

	private rejectAll(message: string): void {
		for (const pending of this.pending.values()) pending.reject(new Error(message));
		this.pending.clear();
	}
}
