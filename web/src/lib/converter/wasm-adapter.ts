import type { ColorMode, DocumentSummary } from './protocol';

interface WasmDocumentSession {
	page_count: number | (() => number);
	inspection: unknown | (() => unknown);
	render_svg(pageIndex: number, colorMode: ColorMode): unknown;
	dispose?: () => void;
	free?: () => void;
}

interface WasmModule {
	default?: (
		moduleOrPath?: { module_or_path: string | URL | Request } | string | URL | Request
	) => Promise<unknown>;
	DocumentSession?: new (bytes: Uint8Array) => WasmDocumentSession;
}

let modulePromise: Promise<WasmModule> | undefined;

async function loadModule(): Promise<WasmModule> {
	modulePromise ??= (async () => {
		const moduleUrl = `${self.location.origin}/wasm/sdocx_wasm.js`;
		const wasmUrl = `${self.location.origin}/wasm/sdocx_wasm_bg.wasm`;
		const module = (await import(/* @vite-ignore */ moduleUrl)) as WasmModule;
		await module.default?.({ module_or_path: wasmUrl });
		return module;
	})();

	return modulePromise;
}

function callOrRead<T>(value: T | (() => T), receiver: object): T {
	return typeof value === 'function' ? (value as () => T).call(receiver) : value;
}

function normalizeInspection(value: unknown): unknown {
	if (typeof value !== 'string') return value;

	try {
		return JSON.parse(value) as unknown;
	} catch {
		return value;
	}
}

function normalizeSvg(value: unknown): string {
	if (typeof value === 'string') return value;
	if (value && typeof value === 'object' && 'svg' in value) {
		const svg = (value as { svg: unknown }).svg;
		if (typeof svg === 'string') return svg;
	}
	throw new Error('The renderer returned an invalid SVG page.');
}

export class BrowserDocumentSession {
	private disposed = false;

	private constructor(private readonly inner: WasmDocumentSession) {}

	static async create(bytes: ArrayBuffer): Promise<BrowserDocumentSession> {
		const module = await loadModule();
		if (!module.DocumentSession) {
			throw new Error('This sdocx WASM build does not include DocumentSession. Rebuild the WASM package.');
		}
		return new BrowserDocumentSession(new module.DocumentSession(new Uint8Array(bytes)));
	}

	summary(): DocumentSummary {
		this.assertActive();
		return {
			pageCount: callOrRead(this.inner.page_count, this.inner),
			inspection: this.inspection()
		};
	}

	inspection(): unknown {
		this.assertActive();
		return normalizeInspection(callOrRead(this.inner.inspection, this.inner));
	}

	renderPage(pageIndex: number, colorMode: ColorMode): string {
		this.assertActive();
		return normalizeSvg(this.inner.render_svg(pageIndex, colorMode));
	}

	dispose(): void {
		if (this.disposed) return;
		this.disposed = true;
		try {
			this.inner.dispose?.();
		} finally {
			this.inner.free?.();
		}
	}

	private assertActive(): void {
		if (this.disposed) throw new Error('The document session has been disposed.');
	}
}
