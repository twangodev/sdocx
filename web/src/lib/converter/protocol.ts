import type { DebugRequest } from '$lib/debugger/model';
import type { ProcessingProgress } from './progress';
export const MAX_INPUT_BYTES = 250 * 1024 * 1024;
export const LARGE_INPUT_BYTES = 100 * 1024 * 1024;

export type ColorMode = 'auto' | 'light' | 'dark';
export type WorkerPhase = 'loading' | 'parsing' | 'inspecting' | 'rendering' | 'ready';

export interface TextRenderDiagnostic {
	kind: string;
	family: string;
	codepoints: number[];
	[field: string]: unknown;
}

export interface ObjectRenderDiagnostic {
	kind: string;
	anchor_utf16: number;
	[field: string]: unknown;
}

export interface GeometryRenderDiagnostic {
	kind: string;
	object_uuid: string;
	source_offset?: number | null;
	[field: string]: unknown;
}

export type PaintRole = 'Fill' | 'Outline';

export interface PaintRenderDiagnostic {
	kind: string;
	object_uuid: string;
	source_offset?: number | null;
	role: PaintRole;
	[field: string]: unknown;
}

export interface PageRenderReport {
	page_index: number;
	source_page_index: number;
	text_diagnostics: TextRenderDiagnostic[];
	object_diagnostics: ObjectRenderDiagnostic[];
	geometry_diagnostics: GeometryRenderDiagnostic[];
	paint_diagnostics: PaintRenderDiagnostic[];
	[field: string]: unknown;
}

export interface SvgRenderResult extends PageRenderReport {
	svg: string;
}

export interface PdfRenderResult {
	bytes: Uint8Array<ArrayBuffer>;
	pages: PageRenderReport[];
	[field: string]: unknown;
}

export interface DocumentSummary {
	pageCount: number;
	inspection: unknown;
}

export type ConverterRequest =
	| { id: number; generation: number; type: 'load'; bytes: ArrayBuffer }
	| { id: number; generation: number; type: 'inspect' }
	| { id: number; generation: number; type: 'debug'; request: DebugRequest }
	| { id: number; generation: number; type: 'renderPage'; pageIndex: number; colorMode: ColorMode }
	| { id: number; generation: number; type: 'exportPdf'; pageIndices: number[]; colorMode: ColorMode }
	| { id: number; generation: number; type: 'resolvePages'; selection: string }
	| { id: number; generation: number; type: 'exportJson' }
	| { id: number; generation: number; type: 'dispose' };

export type ConverterResult =
	| { id: number; type: 'result'; value: DocumentSummary | unknown | string | null }
	| { id: number; type: 'error'; message: string };

export type ConverterEvent =
	| ConverterResult
	| ConverterProgress;

export interface ConverterProgress {
	type: 'progress';
	id: number;
	generation: number;
	operation: 'load' | 'renderPage' | 'exportPdf';
	phase: WorkerPhase;
	message: string;
	progress: ProcessingProgress;
}

export function assertAcceptedFile(file: Pick<File, 'name' | 'size'>): void {
	if (!file.name.toLowerCase().endsWith('.sdocx')) {
		throw new Error('Choose a Samsung Notes .sdocx file.');
	}

	if (file.size === 0) {
		throw new Error('The selected file is empty.');
	}

	if (file.size > MAX_INPUT_BYTES) {
		throw new Error('This file is larger than the 250 MiB browser limit.');
	}
}

export function isLargeFile(file: Pick<File, 'size'>): boolean {
	return file.size > LARGE_INPUT_BYTES;
}
