import { describe, expect, it, vi } from 'vitest';
import type { ConverterClientPort } from './client';
import { DocumentSession } from './document-session.svelte';
import type { DocumentSummary } from './protocol';

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason?: unknown) => void;
	const promise = new Promise<T>((resolvePromise, rejectPromise) => {
		resolve = resolvePromise;
		reject = rejectPromise;
	});
	return { promise, resolve, reject };
}

function file(name: string, bytes: Promise<ArrayBuffer>): File {
	return { name, size: 1, arrayBuffer: () => bytes } as File;
}

function clientWith(load: ConverterClientPort['load']): ConverterClientPort {
	return {
		load,
		inspect: vi.fn(),
		renderPage: vi.fn(),
		exportJson: vi.fn(),
		dispose: vi.fn(),
		cancel: vi.fn(),
		destroy: vi.fn()
	};
}

const emptySummary: DocumentSummary = { pageCount: 0, inspection: {} };

describe('DocumentSession loading', () => {
	it('ignores an older file read that finishes after a newer selection', async () => {
		const olderBytes = deferred<ArrayBuffer>();
		const load = vi.fn(async () => emptySummary);
		const client = clientWith(load);
		const session = new DocumentSession({ createClient: () => client });
		session.start();

		const olderLoad = session.load(file('older.sdocx', olderBytes.promise));
		await session.load(file('newer.sdocx', Promise.resolve(new ArrayBuffer(2))));
		olderBytes.resolve(new ArrayBuffer(1));
		await olderLoad;

		expect(load).toHaveBeenCalledTimes(1);
		expect(session.activeFile?.name).toBe('newer.sdocx');
		expect(session.summary).toBe(emptySummary);
	});

	it('does not restore a document after it is closed during worker loading', async () => {
		const loaded = deferred<DocumentSummary>();
		const client = clientWith(vi.fn(() => loaded.promise));
		const session = new DocumentSession({ createClient: () => client });
		session.start();

		const loading = session.load(file('older.sdocx', Promise.resolve(new ArrayBuffer(1))));
		await Promise.resolve();
		await session.close();
		loaded.resolve(emptySummary);
		await loading;

		expect(session.activeFile).toBeNull();
		expect(session.summary).toBeNull();
		expect(session.status).toBe('Waiting for a document');
	});

	it('propagates a stale close failure without clearing a newer document', async () => {
		const disposed = deferred<void>();
		const client = clientWith(vi.fn(async () => emptySummary));
		client.dispose = vi.fn(() => disposed.promise);
		const session = new DocumentSession({ createClient: () => client });
		session.start();

		const closing = session.close();
		await session.load(file('newer.sdocx', Promise.resolve(new ArrayBuffer(1))));
		const rejection = expect(closing).rejects.toThrow('dispose failed');
		disposed.reject(new Error('dispose failed'));
		await rejection;

		expect(session.activeFile?.name).toBe('newer.sdocx');
		expect(session.summary).toBe(emptySummary);
	});
});
