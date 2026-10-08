import { afterEach, expect, it, vi } from 'vitest';
import { ConverterClient } from './client';
import type { ConverterEvent, ConverterProgress } from './protocol';

afterEach(() => vi.unstubAllGlobals());

it('routes concurrent progress only to its pending request and rejects stale updates', async () => {
	let worker!: FakeWorker;
	class FakeWorker {
		onmessage?: (event: MessageEvent<ConverterEvent>) => void;
		postMessage = vi.fn();
		terminate = vi.fn();
		constructor() { worker = this; }
	}
	vi.stubGlobal('Worker', FakeWorker);
	const global = vi.fn(), render = vi.fn(), exportPdf = vi.fn();
	const client = new ConverterClient(global);
	const loading = client.load(new ArrayBuffer(1), 4);
	const emit = (data: ConverterEvent) => worker.onmessage?.({ data } as MessageEvent<ConverterEvent>);
	emit({ type: 'result', id: 1, value: { pageCount: 1, inspection: {} } });
	await loading;
	const rendering = client.renderPage(0, 'auto', render);
	const exporting = client.exportPdf([0], 'auto', exportPdf);
	const progress: ConverterProgress = { type: 'progress', id: 2, generation: 4, operation: 'renderPage', phase: 'rendering', message: 'Composing vector objects', progress: { stage: 'rendering', completed: 1, total: 3 } };
	emit(progress);
	emit({ ...progress, id: 3, operation: 'exportPdf' });
	emit({ ...progress, generation: 3 });
	emit({ ...progress, id: 99 });
	expect(render).toHaveBeenCalledExactlyOnceWith(progress);
	expect(exportPdf).toHaveBeenCalledTimes(1);
	expect(global).not.toHaveBeenCalled();
	emit({ type: 'result', id: 2, value: {} });
	emit(progress);
	expect(render).toHaveBeenCalledTimes(1);
	emit({ type: 'result', id: 3, value: {} });
	await Promise.all([rendering, exporting]);
	client.destroy();
});
