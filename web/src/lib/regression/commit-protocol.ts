import type { ColorMode } from '$converter/protocol';

export interface RendererArtifact {
	sha: string;
	refs: string[];
	directory: string;
}

export interface RendererCatalog {
	version: 1;
	generatedAt?: string;
	renderers: RendererArtifact[];
}

export interface RenderedPage {
	width: number;
	height: number;
	svg: string;
}

export interface RenderedDocument {
	pageCount: number;
	inspection: unknown;
	pages: RenderedPage[];
}

export type RendererRequest =
	| { id: number; type: 'initialize'; moduleUrl: string; wasmUrl: string }
	| { id: number; type: 'render'; bytes: ArrayBuffer; colorMode: ColorMode };

export type RendererResponse =
	| { id: number; type: 'result'; value: RenderedDocument | null }
	| { id: number; type: 'error'; message: string };
