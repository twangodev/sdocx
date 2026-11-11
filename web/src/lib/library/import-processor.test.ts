import { afterEach, expect, it, vi } from 'vitest';
import type { ConverterClientPort, ProgressListener } from '$converter/client';
import { ImportProcessor } from './import-processor';
import type { LibraryDocument } from './model';

afterEach(() => vi.restoreAllMocks());

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (cause: unknown) => void;
	const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
	return { promise, resolve, reject };
}

function setup() {
	const client: ConverterClientPort = {
		load: vi.fn().mockResolvedValue({ pageCount: 4, inspection: {} }),
		inspect: vi.fn(),
		renderPage: vi.fn<ConverterClientPort['renderPage']>().mockResolvedValue({
			svg: '<svg/>', page_index: 0, source_page_index: 0, text_diagnostics: [], geometry_diagnostics: [], paint_diagnostics: [], background_diagnostics: [],
			object_diagnostics: [{ kind: 'ThumbnailNotice', anchor_utf16: 0 }]
		}),
		exportPdf: vi.fn(),
		resolvePages: vi.fn(),
		exportJson: vi.fn(),
		dispose: vi.fn(),
		cancel: vi.fn(),
		destroy: vi.fn()
	};
	const document = { id: 'saved' } as LibraryDocument;
	const save = vi.fn().mockResolvedValue({ document, duplicate: false });
	const thumbnail = vi.fn().mockResolvedValue(new Blob(['png']));
	const listeners: ProgressListener[] = [];
	const createClient = vi.fn((listener: ProgressListener) => { listeners.push(listener); return client; });
	return { client, save, thumbnail, createClient, listeners, processor: new ImportProcessor(save, createClient, thumbnail) };
}
const file = (name = 'note.sdocx') => new File(['note'], name);

it('imports sequentially, renders only first pages, and continues after invalid files', async () => {
	const { processor, save, client, thumbnail } = setup();
	const results = await processor.run([file(), file('bad.txt'), file('other.sdocx')]);
	expect(results.map((result) => result.status)).toEqual(['imported', 'failed', 'imported']);
	expect(save).toHaveBeenCalledTimes(2);
	expect(save.mock.calls[0][0].contentHash).toMatch(/^[a-f0-9]{64}$/);
	expect(client.renderPage).toHaveBeenCalledTimes(2);
	expect(client.renderPage).toHaveBeenCalledWith(0, 'auto');
	expect(thumbnail).toHaveBeenNthCalledWith(1, '<svg/>');
	expect(thumbnail).toHaveBeenNthCalledWith(2, '<svg/>');
	expect(save.mock.calls[0][0].thumbnail).toBeInstanceOf(Blob);
	expect(await save.mock.calls[0][0].thumbnail.text()).toBe('png');
	expect(client.destroy).toHaveBeenCalledOnce();
});

it('offers a temporary file only after successful parsing and failed persistence', async () => {
	const { processor, save, client } = setup();
	vi.mocked(client.load).mockRejectedValueOnce(new Error('Invalid archive'));
	save.mockRejectedValue(new Error('Storage full'));
	const results = await processor.run([file(), file('valid.sdocx')]);
	expect(results[0]).toMatchObject({ status: 'failed', unsavedFile: undefined });
	expect(results[1]).toMatchObject({ status: 'failed', unsavedFile: expect.any(File) });
});

it('cancellation preserves completed imports and prevents subsequent saves', async () => {
	const { processor, save } = setup();
	const results = await processor.run([file(), file('next.sdocx')], {
		onResult: () => processor.cancel()
	});
	expect(results).toHaveLength(1);
	expect(save).toHaveBeenCalledOnce();
});

it('keeps valid imports when thumbnail rendering fails', async () => {
	const { processor, save, thumbnail } = setup();
	thumbnail.mockRejectedValue(new Error('Cannot rasterize'));
	await processor.run([file()]);
	expect(save.mock.calls[0][0].thumbnail).toBeNull();
});

it('reports duplicates returned by storage', async () => {
	const { processor, save } = setup();
	save.mockResolvedValue({ document: { id: 'existing' }, duplicate: true });
	const results = await processor.run([file()], { collectionId: 'course' });
	expect(results[0].status).toBe('duplicate');
	expect(save.mock.calls[0][1]).toBe('course');
});


it('destroys a cancelled run once without creating a replacement worker or hashing a late read', async () => {
	const { processor, client, save } = setup();
	const bytes = deferred<ArrayBuffer>();
	const input = file();
	vi.spyOn(input, 'arrayBuffer').mockReturnValue(bytes.promise);
	const digest = vi.spyOn(crypto.subtle, 'digest');
	const importing = processor.run([input]);
	processor.cancel();
	expect(client.destroy).toHaveBeenCalledOnce();
	expect(client.cancel).not.toHaveBeenCalled();
	bytes.resolve(new ArrayBuffer(4));
	await expect(importing).resolves.toEqual([]);
	expect(digest).not.toHaveBeenCalled();
	expect(client.load).not.toHaveBeenCalled();
	expect(save).not.toHaveBeenCalled();
	expect(client.destroy).toHaveBeenCalledOnce();
});

it('stops after a pending hash without loading the cancelled file', async () => {
	const { processor, client, save } = setup();
	const digest = deferred<ArrayBuffer>();
	const hashing = vi.spyOn(crypto.subtle, 'digest').mockReturnValue(digest.promise);
	const importing = processor.run([file()]);
	await vi.waitFor(() => expect(hashing).toHaveBeenCalledOnce());
	processor.cancel();
	digest.resolve(new ArrayBuffer(32));
	await expect(importing).resolves.toEqual([]);
	expect(client.load).not.toHaveBeenCalled();
	expect(save).not.toHaveBeenCalled();
});

it('does not encode a thumbnail returned after cancellation', async () => {
	const { processor, client, thumbnail, save } = setup();
	const rendering = deferred<Awaited<ReturnType<ConverterClientPort['renderPage']>>>();
	vi.mocked(client.renderPage).mockReturnValue(rendering.promise);
	const importing = processor.run([file()]);
	await vi.waitFor(() => expect(client.renderPage).toHaveBeenCalledOnce());
	processor.cancel();
	rendering.resolve({ svg: '<svg/>', page_index: 0, source_page_index: 0,
		text_diagnostics: [], geometry_diagnostics: [], paint_diagnostics: [], background_diagnostics: [], object_diagnostics: [] });
	await expect(importing).resolves.toEqual([]);
	expect(thumbnail).not.toHaveBeenCalled();
	expect(save).not.toHaveBeenCalled();
});

it('does not save a thumbnail whose encoding finishes after cancellation', async () => {
	const { processor, thumbnail, save } = setup();
	const encoding = deferred<Blob>();
	thumbnail.mockReturnValue(encoding.promise);
	const importing = processor.run([file()]);
	await vi.waitFor(() => expect(thumbnail).toHaveBeenCalledOnce());
	processor.cancel();
	encoding.resolve(new Blob(['png']));
	await expect(importing).resolves.toEqual([]);
	expect(save).not.toHaveBeenCalled();
});

it('does not begin persistence when cancellation happens in the saving progress callback', async () => {
	const { processor, save } = setup();
	const results = await processor.run([file()], {
		onProgress: (progress) => { if (progress.work?.stage === 'saving') processor.cancel(); }
	});
	expect(results).toEqual([]);
	expect(save).not.toHaveBeenCalled();
});

it.each([false, true])('publishes a completed in-flight save after cancellation (duplicate=%s)', async (duplicate) => {
	const { processor, client, save } = setup();
	const saving = deferred<{ document: LibraryDocument; duplicate: boolean }>();
	save.mockReturnValue(saving.promise);
	const onResult = vi.fn();
	const importing = processor.run([file(), file('next.sdocx')], { onResult });
	await vi.waitFor(() => expect(save).toHaveBeenCalledOnce());
	processor.cancel();
	const document = { id: 'completed' } as LibraryDocument;
	saving.resolve({ document, duplicate });
	const outcome = { filename: 'note.sdocx', status: duplicate ? 'duplicate' : 'imported', document };
	await expect(importing).resolves.toEqual([outcome]);
	expect(onResult).toHaveBeenCalledExactlyOnceWith(outcome);
	expect(save).toHaveBeenCalledOnce();
	expect(client.dispose).not.toHaveBeenCalled();
	expect(client.destroy).toHaveBeenCalledOnce();
});

it('retains completed outcomes when cancellation rejects a pending session dispose', async () => {
	const { processor, client } = setup();
	const disposed = deferred<void>();
	vi.mocked(client.dispose).mockReturnValue(disposed.promise);
	const importing = processor.run([file(), file('next.sdocx')]);
	await vi.waitFor(() => expect(client.dispose).toHaveBeenCalledOnce());
	processor.cancel();
	disposed.reject(new Error('Converter closed.'));
	await expect(importing).resolves.toMatchObject([{ status: 'imported' }]);
	expect(client.load).toHaveBeenCalledOnce();
});

it('rejects overlapping runs until cancelled work settles, then permits a new run', async () => {
	const { processor, createClient } = setup();
	const bytes = deferred<ArrayBuffer>();
	const input = file();
	vi.spyOn(input, 'arrayBuffer').mockReturnValue(bytes.promise);
	const importing = processor.run([input]);
	processor.cancel();
	await expect(processor.run([file('overlap.sdocx')])).rejects.toThrow('already running');
	bytes.resolve(new ArrayBuffer(4));
	await importing;
	await expect(processor.run([file('new.sdocx')])).resolves.toMatchObject([{ status: 'imported' }]);
	expect(createClient).toHaveBeenCalledTimes(2);
});


it('ignores progress owned by a cancelled prior run while a new file is being read', async () => {
	const { processor, listeners } = setup();
	const firstRead = deferred<ArrayBuffer>(), nextRead = deferred<ArrayBuffer>();
	const firstFile = file('first.sdocx'), nextFile = file('next.sdocx');
	vi.spyOn(firstFile, 'arrayBuffer').mockReturnValue(firstRead.promise);
	vi.spyOn(nextFile, 'arrayBuffer').mockReturnValue(nextRead.promise);
	const firstProgress = vi.fn(), nextProgress = vi.fn();
	const first = processor.run([firstFile], { onProgress: firstProgress });
	processor.cancel();
	firstRead.resolve(new ArrayBuffer(4));
	await first;
	const next = processor.run([nextFile], { onProgress: nextProgress });
	const firstCalls = firstProgress.mock.calls.length;
	const nextCalls = nextProgress.mock.calls.length;
	listeners[0]({ type: 'progress', id: 1, generation: 0, operation: 'load', phase: 'parsing',
		message: 'Late prior work', progress: { stage: 'loading', completed: 1, total: 2 } });
	expect(firstProgress).toHaveBeenCalledTimes(firstCalls);
	expect(nextProgress).toHaveBeenCalledTimes(nextCalls);
	nextRead.resolve(new ArrayBuffer(4));
	await expect(next).resolves.toMatchObject([{ status: 'imported' }]);
});

it('releases run admission when client construction fails', async () => {
	const { processor, createClient, client } = setup();
	createClient.mockImplementationOnce(() => { throw new Error('Worker unavailable'); });
	await expect(processor.run([file()])).rejects.toThrow('Worker unavailable');
	await expect(processor.run([file()])).resolves.toMatchObject([{ status: 'imported' }]);
	expect(client.destroy).toHaveBeenCalledOnce();
});

it('preserves a genuine disposal error and permits retry after cleanup', async () => {
	const { processor, client } = setup();
	vi.mocked(client.dispose).mockRejectedValueOnce(new Error('Could not dispose session'));
	await expect(processor.run([file()])).rejects.toThrow('Could not dispose session');
	expect(client.destroy).toHaveBeenCalledOnce();
	await expect(processor.run([file()])).resolves.toMatchObject([{ status: 'imported' }]);
});
