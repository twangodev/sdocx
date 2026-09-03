/// <reference lib="webworker" />

import type {
	RenderedDocument,
	RenderedPage,
	RendererRequest,
	RendererResponse
} from './commit-protocol';

interface WasmDocumentSession {
	page_count: number | (() => number);
	inspection: unknown | (() => unknown);
	render_svg(pageIndex: number, colorMode: string): unknown;
	dispose?: () => void;
	free?: () => void;
}

interface WasmModule {
	default?: (
		moduleOrPath?: { module_or_path: string | URL | Request } | string | URL | Request
	) => Promise<unknown>;
	DocumentSession?: new (bytes: Uint8Array) => WasmDocumentSession;
	render?: (bytes: Uint8Array, darkMode: boolean) => unknown;
	parse?: (bytes: Uint8Array) => unknown;
}

let renderer: WasmModule | undefined;

function emit(response: RendererResponse): void {
	self.postMessage(response);
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

function svgDimensions(svg: string): { width: number; height: number } {
	const viewBox = svg.match(/\bviewBox=["']\s*[-+.\d]+\s+[-+.\d]+\s+([-+.\d]+)\s+([-+.\d]+)\s*["']/i);
	if (viewBox) {
		const width = Number(viewBox[1]);
		const height = Number(viewBox[2]);
		if (width > 0 && height > 0) return { width, height };
	}
	const width = Number(svg.match(/\bwidth=["']([-+.\d]+)/i)?.[1]);
	const height = Number(svg.match(/\bheight=["']([-+.\d]+)/i)?.[1]);
	return {
		width: Number.isFinite(width) && width > 0 ? width : 1,
		height: Number.isFinite(height) && height > 0 ? height : 1
	};
}

function normalizePage(value: unknown): RenderedPage {
	if (typeof value === 'string') return { ...svgDimensions(value), svg: value };
	if (value && typeof value === 'object' && 'svg' in value) {
		const page = value as { svg: unknown; width?: unknown; height?: unknown };
		if (typeof page.svg !== 'string') throw new Error('The renderer returned invalid SVG.');
		const dimensions = svgDimensions(page.svg);
		return {
			width: typeof page.width === 'number' && page.width > 0 ? page.width : dimensions.width,
			height: typeof page.height === 'number' && page.height > 0 ? page.height : dimensions.height,
			svg: page.svg
		};
	}
	throw new Error('The renderer returned an invalid page.');
}

function renderWithSession(module: WasmModule, bytes: Uint8Array, colorMode: string): RenderedDocument {
	if (!module.DocumentSession) throw new Error('DocumentSession is unavailable.');
	const session = new module.DocumentSession(bytes);
	try {
		const pageCount = callOrRead(session.page_count, session);
		const pages = Array.from({ length: pageCount }, (_, pageIndex) =>
			normalizePage(session.render_svg(pageIndex, colorMode))
		);
		return {
			pageCount,
			inspection: normalizeInspection(callOrRead(session.inspection, session)),
			pages
		};
	} finally {
		try {
			session.dispose?.();
		} finally {
			session.free?.();
		}
	}
}

function renderLegacy(module: WasmModule, bytes: Uint8Array, colorMode: string): RenderedDocument {
	if (!module.render) {
		throw new Error('This commit predates the browser renderer contract.');
	}
	const output = module.render(bytes, colorMode === 'dark');
	if (!Array.isArray(output)) throw new Error('The renderer returned an invalid page collection.');
	const pages = output.map(normalizePage);
	return {
		pageCount: pages.length,
		inspection: module.parse ? normalizeInspection(module.parse(bytes)) : null,
		pages
	};
}

async function handle(request: RendererRequest): Promise<RenderedDocument | null> {
	if (request.type === 'initialize') {
		const module = (await import(/* @vite-ignore */ request.moduleUrl)) as WasmModule;
		await module.default?.({ module_or_path: request.wasmUrl });
		renderer = module;
		return null;
	}

	if (!renderer) throw new Error('Initialize the renderer before using it.');
	const bytes = new Uint8Array(request.bytes);
	return renderer.DocumentSession
		? renderWithSession(renderer, bytes, request.colorMode)
		: renderLegacy(renderer, bytes, request.colorMode);
}

self.onmessage = async (event: MessageEvent<RendererRequest>) => {
	const request = event.data;
	try {
		emit({ id: request.id, type: 'result', value: await handle(request) });
	} catch (error) {
		emit({
			id: request.id,
			type: 'error',
			message: error instanceof Error ? error.message : 'The renderer failed.'
		});
	}
};

export {};
