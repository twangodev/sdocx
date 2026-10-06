import { expect, it, vi } from 'vitest';
import type { ConverterClientPort } from '$converter/client';
import { ImportProcessor } from './import-processor';
import type { LibraryDocument } from './model';

function setup() {
	const client: ConverterClientPort = {
		load: vi.fn().mockResolvedValue({ pageCount: 4, inspection: {} }),
		inspect: vi.fn(),
		renderPage: vi.fn<ConverterClientPort['renderPage']>().mockResolvedValue({
			svg: '<svg/>', page_index: 0, source_page_index: 0, text_diagnostics: [], geometry_diagnostics: [], paint_diagnostics: [],
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
	return { client, save, thumbnail, processor: new ImportProcessor(save, () => client, thumbnail) };
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
