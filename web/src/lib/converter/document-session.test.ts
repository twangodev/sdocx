import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ConverterClientPort } from './client';
import { DocumentSession } from './document-session.svelte';
import type { DocumentSummary, PdfRenderResult, SvgRenderResult } from './protocol';
import * as files from './files';

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
		exportPdf: vi.fn(),
		resolvePages: vi.fn(),
		dispose: vi.fn(),
		cancel: vi.fn(),
		destroy: vi.fn()
	};
}

function svgResult(page_index = 0): SvgRenderResult {
	return { svg: '<svg/>', page_index, source_page_index: page_index, text_diagnostics: [], object_diagnostics: [], geometry_diagnostics: [], paint_diagnostics: [] };
}

function pdfResult(indices: number[]): PdfRenderResult {
	return { bytes: new Uint8Array([37, 80, 68, 70]), pages: indices.map(index => ({
		page_index: index, source_page_index: index, text_diagnostics: [], object_diagnostics: [], geometry_diagnostics: [], paint_diagnostics: []
	})) };
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

	it('requests document or current-page PDF bytes from the worker', async () => {
		const client = clientWith(vi.fn(async () => ({ pageCount: 2, inspection: {} })));
		client.renderPage = vi.fn(async index => svgResult(index));
		client.exportPdf = vi.fn(async indices => pdfResult(indices));
		const session = new DocumentSession({ createClient: () => client });
		session.start();
		await session.load(file('note.sdocx', Promise.resolve(new ArrayBuffer(1))));
		const download = vi.spyOn(files, 'downloadBlob').mockImplementation(() => {});
		session.colorMode = 'dark';
		await session.downloadExport({ format: 'pdf', pageIndices: [0, 1], pngScale: 1 });
		await session.downloadExport({ format: 'pdf', pageIndices: [1], pngScale: 1 });
		expect(vi.mocked(client.exportPdf).mock.calls).toEqual([[[0, 1], 'dark'], [[1], 'dark']]);
		expect(download).toHaveBeenNthCalledWith(1, expect.any(Blob), 'note.pdf');
		expect(download).toHaveBeenNthCalledWith(2, expect.any(Blob), 'note-page-002.pdf');
		expect(download.mock.calls[0][0].type).toBe('application/pdf');
		expect(session.exporting).toBe(false);
		session.destroy();
	});

	it('does not download a PDF that finishes after cancellation', async () => {
		const client = clientWith(vi.fn(async () => ({ pageCount: 1, inspection: {} })));
		client.renderPage = vi.fn(async index => svgResult(index));
		const session = new DocumentSession({ createClient: () => client });
		session.start();
		await session.load(file('note.sdocx', Promise.resolve(new ArrayBuffer(1))));
		const download = vi.spyOn(files, 'downloadBlob').mockImplementation(() => {});
		const started = deferred<void>();
		const converted = deferred<PdfRenderResult>();
		vi.mocked(client.exportPdf).mockImplementationOnce(async () => {
			started.resolve();
			return converted.promise;
		});
		const exporting = session.downloadExport({ format: 'pdf', pageIndices: [0], pngScale: 1 });
		await started.promise;
		session.cancel();
		converted.resolve(pdfResult([0]));
		await exporting;
		expect(download).not.toHaveBeenCalled();
		expect(session.exporting).toBe(false);
		expect(session.exportReports).toEqual([]);
		session.destroy();
	});
});

describe('render report scopes', () => {
	it('collects ZIP reports once per rendered page while reusing SVG for PNG output', async () => {
		const client = clientWith(vi.fn(async () => ({ pageCount: 2, inspection: {} })));
		client.renderPage = vi.fn(async index => ({ ...svgResult(index), object_diagnostics: [{ kind: 'ArchiveNotice', anchor_utf16: index }] }));
		client.exportJson = vi.fn(async () => '{}');
		vi.spyOn(files, 'svgToPng').mockResolvedValue(new Blob(['png'], { type: 'image/png' }));
		const download = vi.spyOn(files, 'downloadBlob').mockImplementation(() => {});
		const session = new DocumentSession({ createClient: () => client });
		session.start();
		await session.load(file('note.sdocx', Promise.resolve(new ArrayBuffer(1))));
		vi.mocked(client.renderPage).mockClear();
		await session.downloadExport({ format: 'everything', pageIndices: [], pngScale: 1, colorMode: 'dark' });
		expect(vi.mocked(client.renderPage).mock.calls).toEqual([[0, 'dark'], [1, 'dark']]);
		expect(files.svgToPng).toHaveBeenCalledTimes(2);
		expect(session.exportReports.map(report => report.page_index)).toEqual([0, 1]);
		const { unzipSync } = await import('fflate');
		const entries = unzipSync(new Uint8Array(await download.mock.calls[0][0].arrayBuffer()));
		expect(Object.keys(entries)).toEqual(['document.json', 'manifest.json', 'note-page-001.svg', 'note-page-001.png', 'note-page-002.svg', 'note-page-002.png']);
		session.destroy();
	});

	it('keeps viewer, thumbnail, export preview, and ordered PDF reports separate', async () => {
		const client = clientWith(vi.fn(async () => ({ pageCount: 2, inspection: {} })));
		client.renderPage = vi.fn(async index => ({ ...svgResult(index), object_diagnostics: [{ kind: 'ViewerNotice', anchor_utf16: 0 }] }));
		const session = new DocumentSession({ createClient: () => client });
		session.start();
		const source = file('note.sdocx', Promise.resolve(new ArrayBuffer(1)));
		await session.load(source);
		const viewerReports = [...session.previewReports];
		client.renderPage = vi.fn(async index => ({ ...svgResult(index), object_diagnostics: [{ kind: 'OtherNotice', anchor_utf16: 1 }] }));
		expect((await session.renderExportPreview(1, 'dark')).object_diagnostics[0].kind).toBe('OtherNotice');
		expect(await session.renderThumbnailSvg(source)).toBe('<svg/>');
		expect(session.previewReports).toEqual(viewerReports);
		expect(session.exportReports).toEqual([]);
		client.exportPdf = vi.fn(async (indices: number[]) => {
			const result = pdfResult(indices);
			return { ...result, pages: result.pages.map(report => ({
				...report, object_diagnostics: [{ kind: 'DownloadNotice', anchor_utf16: report.page_index }]
			})) };
		});
		vi.spyOn(files, 'downloadBlob').mockImplementation(() => {});
		await session.downloadExport({ format: 'pdf', pageIndices: [1, 0, 1], pngScale: 1, colorMode: 'dark' });
		expect(session.exportReports.map(report => report.page_index)).toEqual([1, 0, 1]);
		expect(session.exportReports.every(report => report.object_diagnostics[0].kind === 'DownloadNotice')).toBe(true);
		expect(session.exportColorMode).toBe('dark');
		expect(session.previewReports).toEqual(viewerReports);
		vi.mocked(client.exportPdf).mockRejectedValueOnce(new Error('PDF failed'));
		await session.downloadExport({ format: 'pdf', pageIndices: [0], pngScale: 1 });
		expect(session.error).toBe('PDF failed');
		expect(session.exportReports).toEqual([]);
		await session.close();
		expect(session.previewReports).toEqual([]);
		expect(session.exportReports).toEqual([]);
		session.destroy();
	});

	it('does not publish stale theme reports or rendered pages', async () => {
		const client = clientWith(vi.fn(async () => ({ pageCount: 2, inspection: {} })));
		client.renderPage = vi.fn(async index => svgResult(index));
		const onPageRendered = vi.fn();
		const session = new DocumentSession({ createClient: () => client, onPageRendered });
		session.start();
		await session.load(file('note.sdocx', Promise.resolve(new ArrayBuffer(1))));
		onPageRendered.mockClear();
		const darkPage = deferred<SvgRenderResult>();
		client.renderPage = vi.fn(async (index, mode) => mode === 'dark' ? darkPage.promise : {
			...svgResult(index), object_diagnostics: [{ kind: 'LightNotice', anchor_utf16: 0 }]
		});
		const darkRender = session.setColorMode('dark');
		await session.setColorMode('light');
		const urls = [...session.previewUrls];
		darkPage.resolve({ ...svgResult(), object_diagnostics: [{ kind: 'StaleDarkNotice', anchor_utf16: 0 }] });
		await darkRender;
		expect(session.previewUrls).toEqual(urls);
		expect(session.previewColorMode).toBe('light');
		expect(session.previewReports.map(report => report.object_diagnostics[0].kind)).toEqual(['LightNotice', 'LightNotice']);
		expect(onPageRendered).toHaveBeenCalledTimes(2);
		expect(client.renderPage).toHaveBeenCalledTimes(3);
		session.destroy();
	});

	it('snapshots viewer colors for all pages in an in-flight render', async () => {
		const client = clientWith(vi.fn(async () => ({ pageCount: 2, inspection: {} })));
		client.renderPage = vi.fn(async index => svgResult(index));
		const session = new DocumentSession({ createClient: () => client });
		session.start();
		await session.load(file('note.sdocx', Promise.resolve(new ArrayBuffer(1))));
		const first = deferred<SvgRenderResult>();
		vi.mocked(client.renderPage).mockClear().mockReturnValueOnce(first.promise);
		const rendering = session.setColorMode('dark');
		session.colorMode = 'light';
		first.resolve(svgResult());
		await rendering;
		expect(vi.mocked(client.renderPage).mock.calls).toEqual([[0, 'dark'], [1, 'dark']]);
		expect(session.previewColorMode).toBe('dark');
		session.destroy();
	});
});

it('snapshots archive pages and colors while settings change', async () => {
	const client = clientWith(vi.fn(async () => ({ pageCount: 3, inspection: {} })));
	client.renderPage = vi.fn(async index => svgResult(index));
	const session = new DocumentSession({ createClient: () => client });
	session.start();
	await session.load(file('note.sdocx', Promise.resolve(new ArrayBuffer(1))));
	const download = vi.spyOn(files, 'downloadBlob').mockImplementation(() => {});
	const rendered = deferred<SvgRenderResult>();
	const started = deferred<void>();
	vi.mocked(client.renderPage).mockClear().mockImplementationOnce(async () => {
		started.resolve();
		return rendered.promise;
	});
	session.colorMode = 'dark';
	const indices = [0, 2];
	const exporting = session.downloadExport({ format: 'svg', pageIndices: indices, pngScale: 1 });
	await started.promise;
	indices.splice(0, 2, 1);
	session.colorMode = 'light';
	rendered.resolve(svgResult());
	await exporting;
	expect(vi.mocked(client.renderPage).mock.calls).toEqual([[0, 'dark'], [2, 'dark']]);
	expect(download).toHaveBeenCalledWith(expect.any(Blob), 'note-selected-svg.zip');
	const { unzipSync } = await import('fflate');
	const entries = unzipSync(new Uint8Array(await download.mock.calls[0][0].arrayBuffer()));
	expect(Object.keys(entries)).toEqual(['note-page-001.svg', 'note-page-003.svg']);
	session.destroy();
});

it('publishes a rendered first page before later pages finish and ignores cancelled renders', async () => {
	const laterPage = deferred<SvgRenderResult>();
	const client = clientWith(vi.fn(async () => ({ pageCount: 2, inspection: {} })));
	vi.mocked(client.renderPage).mockResolvedValueOnce(svgResult()).mockReturnValueOnce(laterPage.promise);
	const onPageRendered = vi.fn();
	const session = new DocumentSession({ createClient: () => client, onPageRendered });
	const stop = session.start();
	const input = file('note.sdocx', Promise.resolve(new ArrayBuffer(1)));
	const loading = session.load(input);
	await vi.waitFor(() => expect(onPageRendered).toHaveBeenCalledWith({ file: input, pageIndex: 0, svg: '<svg/>' }));
	await session.close();
	laterPage.resolve(svgResult(1));
	await loading;
	expect(onPageRendered).toHaveBeenCalledOnce();
	stop();
});


describe('canonical thumbnail rendering', () => {
	it('uses Auto regardless of the active viewer mode', async () => {
		const client = clientWith(vi.fn(async () => emptySummary));
		client.renderPage = vi.fn(async index => svgResult(index));
		const session = new DocumentSession({ createClient: () => client });
		session.start();
		const source = file('note.sdocx', Promise.resolve(new ArrayBuffer(1)));
		await session.load(source);
		for (const mode of ['dark', 'light', 'auto'] as const) {
			session.colorMode = mode;
			expect(await session.renderThumbnailSvg(source)).toBe('<svg/>');
			expect(client.renderPage).toHaveBeenLastCalledWith(0, 'auto');
		}
		session.destroy();
	});

	it('rejects thumbnail results after the document is replaced', async () => {
		const rendered = deferred<SvgRenderResult>();
		const client = clientWith(vi.fn(async () => emptySummary));
		client.renderPage = vi.fn(() => rendered.promise);
		const session = new DocumentSession({ createClient: () => client });
		session.start();
		const source = file('old.sdocx', Promise.resolve(new ArrayBuffer(1)));
		await session.load(source);
		const pending = expect(session.renderThumbnailSvg(source)).rejects.toThrow('Document replaced');
		await session.load(file('new.sdocx', Promise.resolve(new ArrayBuffer(1))));
		rendered.resolve(svgResult());
		await pending;
		await expect(session.renderThumbnailSvg(source)).rejects.toThrow('Document replaced');
		session.destroy();
	});
});


describe('export preview colors', () => {
	it('previews and exports in an independent color mode', async () => {
		const client = clientWith(vi.fn(async () => ({ pageCount: 2, inspection: {} })));
		client.renderPage = vi.fn(async index => svgResult(index));
		client.exportPdf = vi.fn(async indices => pdfResult(indices));
		vi.spyOn(files, 'downloadBlob').mockImplementation(() => {});
		const session = new DocumentSession({ createClient: () => client });
		session.start();
		await session.load(file('note.sdocx', Promise.resolve(new ArrayBuffer(1))));
		session.colorMode = 'light';
		expect(await session.renderExportPreview(1, 'dark')).toEqual(svgResult(1));
		expect(client.renderPage).toHaveBeenLastCalledWith(1, 'dark');
		await session.downloadExport({ format: 'pdf', pageIndices: [1], pngScale: 1, colorMode: 'dark' });
		expect(client.exportPdf).toHaveBeenLastCalledWith([1], 'dark');
		await session.downloadExport({ format: 'svg', pageIndices: [1], pngScale: 1, colorMode: 'dark' });
		expect(client.renderPage).toHaveBeenLastCalledWith(1, 'dark');
		expect(session.colorMode).toBe('light');
		session.destroy();
	});

	it('rejects invalid pages and previews completed after document replacement', async () => {
		const client = clientWith(vi.fn(async () => ({ pageCount: 1, inspection: {} })));
		client.renderPage = vi.fn(async index => svgResult(index));
		const session = new DocumentSession({ createClient: () => client });
		session.start();
		await session.load(file('note.sdocx', Promise.resolve(new ArrayBuffer(1))));
		await expect(session.renderExportPreview(1, 'auto')).rejects.toThrow('valid preview page');
		const result = deferred<SvgRenderResult>();
		client.renderPage = vi.fn(() => result.promise);
		const pending = expect(session.renderExportPreview(0, 'dark')).rejects.toThrow('Document replaced');
		session.cancel();
		result.resolve(svgResult());
		await pending;
		session.destroy();
	});
});
