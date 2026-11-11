import 'fake-indexeddb/auto';
import { afterEach, expect, it, vi } from 'vitest';
import type { ConverterClientPort } from '$converter/client';
import { LibraryWorkspace } from './workspace.svelte';
import { ImportProcessor } from './import-processor';

function deferred() {
	let resolve!: () => void;
	const promise = new Promise<void>((yes) => { resolve = yes; });
	return { promise, resolve };
}
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });

for (const restart of [false, true]) {
	it(`preserves an admitted save after stop${restart ? ' and restart' : ' without restart'}`, async () => {
		vi.stubGlobal('window', new EventTarget());
		let lockTail = Promise.resolve();
		vi.stubGlobal('navigator', { storage: { getDirectory() {} }, locks: { request: (_name: string, action: () => unknown) => {
			const operation = lockTail.then(action);
			lockTail = operation.then(() => {}, () => {});
			return operation;
		} } });
		vi.stubGlobal('BroadcastChannel', undefined);
		const workspace = new LibraryWorkspace();
		const catalog = workspace.service.catalog;
		await catalog.delete();
		const closing = vi.spyOn(workspace.service, 'closeWhenIdle');
		const assets = new Map<string, Blob>();
		vi.spyOn(workspace.service.assets, 'list').mockImplementation(async () => [...assets.keys()]);
		vi.spyOn(workspace.service.assets, 'remove').mockImplementation(async (name) => { assets.delete(name); });
		const writing = deferred(), release = deferred();
		vi.spyOn(workspace.service.assets, 'write').mockImplementation(async (name, contents) => {
			writing.resolve(); await release.promise; assets.set(name, contents);
		});
		const client: ConverterClientPort = {
			load: vi.fn().mockResolvedValue({ pageCount: 0, inspection: {} }),
			inspect: vi.fn(), renderPage: vi.fn(), exportPdf: vi.fn(), resolvePages: vi.fn(),
			exportJson: vi.fn(), dispose: vi.fn(), cancel: vi.fn(), destroy: vi.fn()
		};
		let saveError: string | null = null;
		Object.defineProperty(workspace, 'importer', { value: new ImportProcessor(async (prepared) => {
			try { return await workspace.service.importDocument(prepared); }
			catch (error) { saveError = (error as Error).name; throw error; }
		}, () => client) });
		let stop = workspace.start();
		try {
			await workspace.importFiles([]);
			const importing = workspace.importFiles([new File(['note'], 'note.sdocx'), new File(['next'], 'next.sdocx')]);
			await writing.promise;
			stop();
			if (restart) { stop = workspace.start(); expect(workspace.loading).toBe(true); }
			release.resolve();
			const results = await importing;
			await closing.mock.results[0].value;
			expect(catalog.isOpen()).toBe(restart);
			if (restart) await workspace.importFiles([]);
			expect(client.load).toHaveBeenCalledOnce();
			// Reopening for inspection must happen after the save settles.
			await catalog.open();
			const documents = await catalog.documents.toArray();
			expect({ statuses: results.map((result) => result.status), documents: documents.length, assets: assets.size, saveError })
				.toEqual({ statuses: ['imported'], documents: 1, assets: 1, saveError: null });
		} finally { release.resolve(); stop(); await closing.mock.results.at(-1)?.value; await catalog.delete(); }
	});
}
