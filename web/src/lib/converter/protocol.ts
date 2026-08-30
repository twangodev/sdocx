export const MAX_INPUT_BYTES = 250 * 1024 * 1024;
export const LARGE_INPUT_BYTES = 100 * 1024 * 1024;

export type ColorMode = 'auto' | 'light' | 'dark';
export type WorkerPhase = 'loading' | 'parsing' | 'inspecting' | 'rendering' | 'ready';

export interface DocumentSummary {
	pageCount: number;
	inspection: unknown;
}

export type ConverterRequest =
	| { id: number; type: 'load'; bytes: ArrayBuffer }
	| { id: number; type: 'inspect' }
	| { id: number; type: 'renderPage'; pageIndex: number; colorMode: ColorMode }
	| { id: number; type: 'exportJson' }
	| { id: number; type: 'dispose' };

export type ConverterResult =
	| { id: number; type: 'result'; value: DocumentSummary | unknown | string | null }
	| { id: number; type: 'error'; message: string };

export type ConverterEvent =
	| ConverterResult
	| { type: 'progress'; phase: WorkerPhase; message: string };

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
