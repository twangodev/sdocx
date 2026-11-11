import { afterEach, expect, it, vi } from 'vitest';
import { DocumentSession } from './document-session.svelte';
import type { ConverterEvent, ConverterRequest } from './protocol';

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });

it('clears document resources when replacement worker creation fails while preserving failure and recovery', async () => {
	let constructions = 0;
	const workers: ControlledWorker[] = [];
	class ControlledWorker {
		onmessage?: (event: MessageEvent<ConverterEvent>) => void;
		onerror: unknown;
		terminate = vi.fn();
		constructor() {
			if (++constructions === 2) throw new Error('Worker allocation failed');
			workers.push(this);
		}
		postMessage(request: ConverterRequest) {
			const value = request.type === 'load'
				? { pageCount: 1, inspection: {} }
				: { svg: '<svg/>', page_index: 0, source_page_index: 0, text_diagnostics: [], object_diagnostics: [], geometry_diagnostics: [], paint_diagnostics: [], background_diagnostics: [] };
			queueMicrotask(() => this.onmessage?.({ data: { id: request.id, type: 'result', value } } as MessageEvent<ConverterEvent>));
		}
	}
	vi.stubGlobal('Worker', ControlledWorker);
	const revoke = vi.spyOn(URL, 'revokeObjectURL');
	const session = new DocumentSession();
	session.start();
	try {
		await session.load(new File(['note'], 'note.sdocx'));
		expect(session.hasDocument).toBe(true);
		const preview = session.previewUrls[0];
		expect(preview).toBeTruthy();
		expect(() => session.cancel()).toThrow('Worker allocation failed');
		expect(workers[0].terminate).toHaveBeenCalledOnce();
		const afterFailure = {
			hasDocument: session.hasDocument,
			previewUrls: [...session.previewUrls],
			previewRevoked: revoke.mock.calls.some(([url]) => url === preview),
			parsing: session.parsing, rendering: session.rendering, exporting: session.exporting,
			progress: session.progress, exportWork: session.exportWork, status: session.status
		};
		await expect(session.resolvePages('1')).rejects.toThrow('Processing cancelled.');
		session.cancel();
		await session.load(new File(['reopened'], 'reopened.sdocx'));
		expect(session.hasDocument).toBe(true);
		expect(workers).toHaveLength(2);
		expect(afterFailure).toEqual({ hasDocument: false, previewUrls: [], previewRevoked: true,
			parsing: false, rendering: false, exporting: false, progress: null, exportWork: null, status: 'Processing cancelled' });
	} finally {
		session.destroy();
	}
});
