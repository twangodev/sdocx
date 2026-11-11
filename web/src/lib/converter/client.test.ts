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

function workerHarness() {
	const workers: FakeWorker[] = [];
	class FakeWorker {
		onmessage?: (event: MessageEvent<ConverterEvent>) => void;
		onerror?: (event: ErrorEvent) => void;
		postMessage = vi.fn();
		terminate = vi.fn();
		constructor() { workers.push(this); }
	}
	vi.stubGlobal('Worker', FakeWorker);
	return workers;
}

it('rejects requests after destruction without posting to a terminated worker', async () => {
	const workers = workerHarness();
	const client = new ConverterClient();
	client.destroy();
	const rejected = vi.fn();
	void client.inspect().catch(rejected);
	await vi.waitFor(() => expect(rejected).toHaveBeenCalledOnce(), { timeout: 100, interval: 5 });
	expect(rejected.mock.calls[0][0]).toEqual(new Error('Converter closed.'));
	expect(workers[0].postMessage).not.toHaveBeenCalled();
});

it('rejects future requests after a worker error instead of leaving them pending', async () => {
	const workers = workerHarness();
	const client = new ConverterClient();
	workers[0].onerror?.({ message: 'worker failed' } as ErrorEvent);
	const rejected = vi.fn();
	void client.inspect().catch(rejected);
	await vi.waitFor(() => expect(rejected).toHaveBeenCalledOnce(), { timeout: 100, interval: 5 });
	expect(rejected.mock.calls[0][0]).toEqual(new Error('worker failed'));
	expect(workers[0].postMessage).not.toHaveBeenCalled();
	client.destroy();
});

it('forgets a request when postMessage throws synchronously', async () => {
	const workers = workerHarness();
	const client = new ConverterClient();
	const progress = vi.fn();
	workers[0].postMessage.mockImplementationOnce(() => { throw new Error('could not post'); });
	await expect(client.renderPage(0, 'auto', progress)).rejects.toThrow('could not post');
	workers[0].onmessage?.({ data: {
		type: 'progress', id: 1, generation: 0, operation: 'renderPage', phase: 'rendering',
		message: 'late progress', progress: { stage: 'rendering', completed: 1, total: 1 }
	} } as MessageEvent<ConverterEvent>);
	expect(progress).not.toHaveBeenCalled();
	const next = client.inspect();
	workers[0].onmessage?.({ data: { type: 'result', id: 2, value: { recovered: true } } } as MessageEvent<ConverterEvent>);
	await expect(next).resolves.toEqual({ recovered: true });
	client.destroy();
});

it('ignores errors from a replaced worker while new requests are pending', async () => {
	const workers = workerHarness();
	const client = new ConverterClient();
	const staleError = workers[0].onerror;
	client.cancel();
	const result = client.inspect();
	staleError?.({ message: 'old worker failed' } as ErrorEvent);
	workers[1].onmessage?.({ data: { type: 'result', id: 1, value: { current: true } } } as MessageEvent<ConverterEvent>);
	await expect(result).resolves.toEqual({ current: true });
	client.destroy();
});


it('recovers a failed worker through cancellation and never resurrects a destroyed client', async () => {
	const workers = workerHarness();
	const client = new ConverterClient();
	const initial = client.inspect();
	const rejected = expect(initial).rejects.toThrow('worker failed');
	workers[0].onerror?.({ message: 'worker failed' } as ErrorEvent);
	await rejected;
	expect(workers[0].terminate).toHaveBeenCalledOnce();
	client.cancel();
	expect(workers).toHaveLength(2);
	const recovered = client.inspect();
	workers[1].onmessage?.({ data: { type: 'result', id: 2, value: { recovered: true } } } as MessageEvent<ConverterEvent>);
	await expect(recovered).resolves.toEqual({ recovered: true });
	client.destroy();
	client.cancel();
	client.destroy();
	expect(workers).toHaveLength(2);
	expect(workers[1].terminate).toHaveBeenCalledOnce();
	await expect(client.inspect()).rejects.toThrow('Converter closed.');
});
