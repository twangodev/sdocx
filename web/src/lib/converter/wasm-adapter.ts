import type { DebugRequest } from '$lib/debugger/model';
import type { ColorMode, DocumentSummary, PdfRenderResult, SvgRenderResult } from './protocol';
import { validatePdfResult, validateSvgResult } from './render-reports';

interface WasmDocumentSession {
	page_count: number | (() => number);
	inspection: unknown | (() => unknown);
	debug?: (request: string) => string;
	resolve_pages(selection: string): Uint32Array;
	render_pdf_pages_detailed(pageIndices: Uint32Array, colorMode: ColorMode): unknown;
	render_svg_detailed(pageIndex: number, colorMode: ColorMode): unknown;
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

	renderPage(pageIndex: number, colorMode: ColorMode): SvgRenderResult {
		this.assertActive();
		if (typeof this.inner.render_svg_detailed !== 'function') {
			throw new Error('Rebuild the WASM package to enable SVG rendering reports.');
		}
		return validateSvgResult(this.inner.render_svg_detailed(pageIndex, colorMode), pageIndex);
	}

	resolvePages(selection: string): number[] {
		this.assertActive();
		return Array.from(this.inner.resolve_pages(selection));
	}

	async exportPdf(pageIndices: number[], colorMode: ColorMode): Promise<PdfRenderResult> {
		this.assertActive();
		if (typeof this.inner.render_pdf_pages_detailed !== 'function') {
			throw new Error('Rebuild the WASM package to enable PDF rendering reports.');
		}
		return validatePdfResult(this.inner.render_pdf_pages_detailed(new Uint32Array(pageIndices), colorMode), pageIndices);
	}

	debug(request: DebugRequest): unknown {
		this.assertActive();
		if (!this.inner.debug) throw new Error("Rebuild WASM to enable the debugger.");
		return JSON.parse(this.inner.debug(JSON.stringify(request)));
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
