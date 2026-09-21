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

it('routes debugger requests only to the current session', async () => {
 const session = { ...fakeSession('debug'), debug: vi.fn(() => ({ offset: 42 })) };
 const worker = new ConverterWorkerSession(vi.fn(), async () => session);
 await worker.handle({ id: 1, generation: 2, type: 'load', bytes: new ArrayBuffer(1) });
 const request = { kind: 'object', page: 0, offset: 42 } as const;
 await expect(worker.handle({ id: 2, generation: 2, type: 'debug', request })).resolves.toEqual({ offset: 42 });
 await expect(worker.handle({ id: 3, generation: 1, type: 'debug', request })).rejects.toThrow(/superseded/);
 expect(session.debug).toHaveBeenCalledTimes(1);
 await worker.handle({ id: 4, generation: 3, type: 'dispose' });
 await expect(worker.handle({ id: 5, generation: 3, type: 'debug', request })).rejects.toThrow(/Load a document/);
});
