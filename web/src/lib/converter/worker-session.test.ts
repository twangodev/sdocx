import { describe, expect, it, vi } from 'vitest';
import { ConverterWorkerSession } from './worker-session';

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((resolvePromise) => {
		resolve = resolvePromise;
	});
	return { promise, resolve };
}

function fakeSession(label: string) {
	return {
		summary: () => ({ pageCount: 1, inspection: { label } }),
		inspection: () => ({ label }),
		renderPage: () => `<svg>${label}</svg>`,
		dispose: vi.fn()
	};
}

describe('ConverterWorkerSession generations', () => {
	it('keeps the newest session when an older load finishes last', async () => {
		const older = deferred<ReturnType<typeof fakeSession>>();
		const newer = deferred<ReturnType<typeof fakeSession>>();
		const createSession = vi
			.fn<(bytes: ArrayBuffer) => Promise<ReturnType<typeof fakeSession>>>()
			.mockReturnValueOnce(older.promise)
			.mockReturnValueOnce(newer.promise);
		const worker = new ConverterWorkerSession(vi.fn(), createSession);
		const olderLoad = worker.handle({
			id: 1,
			generation: 1,
			type: 'load',
			bytes: new ArrayBuffer(1)
		});
		const newerLoad = worker.handle({
			id: 2,
			generation: 2,
			type: 'load',
			bytes: new ArrayBuffer(2)
		});
		const newerSession = fakeSession('newer');
		newer.resolve(newerSession);
		await expect(newerLoad).resolves.toEqual({ pageCount: 1, inspection: { label: 'newer' } });
		const olderSession = fakeSession('older');
		older.resolve(olderSession);
		await expect(olderLoad).rejects.toThrow(/superseded/i);

		await expect(
			worker.handle({ id: 3, generation: 2, type: 'renderPage', pageIndex: 0, colorMode: 'auto' })
		).resolves.toBe('<svg>newer</svg>');
		expect(olderSession.dispose).toHaveBeenCalledOnce();
		expect(newerSession.dispose).not.toHaveBeenCalled();
	});

	it('disposes a session that finishes after a newer close', async () => {
		const pending = deferred<ReturnType<typeof fakeSession>>();
		const worker = new ConverterWorkerSession(vi.fn(), () => pending.promise);
		const loading = worker.handle({
			id: 1,
			generation: 1,
			type: 'load',
			bytes: new ArrayBuffer(1)
		});
		await worker.handle({ id: 2, generation: 2, type: 'dispose' });
		const staleSession = fakeSession('stale');
		pending.resolve(staleSession);

		await expect(loading).rejects.toThrow(/superseded/i);
		expect(staleSession.dispose).toHaveBeenCalledOnce();
	});
});
