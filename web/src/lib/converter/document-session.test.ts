import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ConverterClientPort } from './client';
import { DocumentSession } from './document-session.svelte';
import type { DocumentSummary } from './protocol';
import * as files from './files';
import { createPdf } from './pdf';

vi.mock('./pdf', () => ({ createPdf: vi.fn() }));
afterEach(() => vi.restoreAllMocks());

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

describe('DocumentSession PDF exports', () => {
	it('renders every page in order using the captured document color mode', async () => {
		const client = clientWith(vi.fn(async () => ({ pageCount: 2, inspection: {} })));
		client.renderPage = vi.fn(async (index) => `<svg>${index}</svg>`);
		const session = new DocumentSession({ createClient: () => client });
		session.start();
		await session.load(file('note.sdocx', Promise.resolve(new ArrayBuffer(1))));
		vi.mocked(client.renderPage).mockClear();
		const download = vi.spyOn(files, 'downloadBlob').mockImplementation(() => {});
		const converted: string[] = [];
		vi.mocked(createPdf).mockImplementationOnce(async (pages) => {
			for await (const svg of pages) converted.push(svg);
			return new Blob(['pdf'], { type: 'application/pdf' });
		});
		session.colorMode = 'dark';
		await session.downloadPdf();
		expect(converted).toEqual(['<svg>0</svg>', '<svg>1</svg>']);
		expect(vi.mocked(client.renderPage).mock.calls).toEqual([[0, 'dark'], [1, 'dark']]);
		expect(download).toHaveBeenCalledWith(expect.any(Blob), 'note.pdf');
		expect(session.exporting).toBe(false);
		session.destroy();
	});

	it('does not download a PDF that finishes after cancellation', async () => {
		const client = clientWith(vi.fn(async () => ({ pageCount: 1, inspection: {} })));
		client.renderPage = vi.fn(async () => '<svg/>');
		const session = new DocumentSession({ createClient: () => client });
		session.start();
		await session.load(file('note.sdocx', Promise.resolve(new ArrayBuffer(1))));
		const download = vi.spyOn(files, 'downloadBlob').mockImplementation(() => {});
		const started = deferred<void>();
		const converted = deferred<Blob>();
		vi.mocked(createPdf).mockImplementationOnce(async () => {
			started.resolve();
			return converted.promise;
		});
		const exporting = session.downloadPdf(0);
		await started.promise;
		session.cancel();
		converted.resolve(new Blob(['pdf']));
		await exporting;
		expect(download).not.toHaveBeenCalled();
		expect(session.exporting).toBe(false);
		session.destroy();
	});
});
