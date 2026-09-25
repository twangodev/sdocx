import 'fake-indexeddb/auto';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { LibraryCatalog } from './catalog';
import { LibraryService } from './service';
import type { AssetStore } from './asset-store';
import type { PreparedDocument } from './model';

class MemoryAssets implements AssetStore {
	files = new Map<string, File>();
	async read(name: string) { const file = this.files.get(name); if (!file) throw new Error('Missing asset'); return file; }
	async write(name: string, blob: Blob) { this.files.set(name, new File([blob], name)); }
	async remove(name: string) { this.files.delete(name); }
	async list() { return [...this.files.keys()]; }
}

const catalogs: LibraryCatalog[] = [];
function setup() {
	const catalog = new LibraryCatalog(`test-${crypto.randomUUID()}`);
	catalogs.push(catalog);
	const assets = new MemoryAssets();
	const service = new LibraryService(catalog, assets, (action) => action());
	return { catalog, assets, service };
}
function note(contentHash = 'hash', filename = 'note.sdocx'): PreparedDocument {
	return { file: new File(['note'], filename), contentHash, title: 'Note', pageCount: 2, thumbnail: new Blob(['png']) };
}
afterEach(async () => { for (const catalog of catalogs.splice(0)) await catalog.delete(); });

describe('library persistence', () => {
	it('deduplicates content and adds existing notes to collections', async () => {
		const { service, catalog, assets } = setup();
		const first = await service.importDocument(note());
		const collection = await service.saveCollection('Course');
		const duplicate = await service.importDocument(note('hash', 'renamed.sdocx'), collection.id);
		expect(duplicate).toEqual({ document: first.document, duplicate: true });
		expect(await catalog.memberships.toArray()).toEqual([{ collectionId: collection.id, documentId: first.document.id }]);
		expect(assets.files.size).toBe(2);
		await service.importDocument(note('changed'));
		expect(await catalog.documents.count()).toBe(2);
	});

	it('preserves notes when deleting a collection and removes assets when deleting notes', async () => {
		const { service, catalog, assets } = setup();
		const collection = await service.saveCollection('Course');
		const { document } = await service.importDocument(note(), collection.id);
		await service.deleteCollection(collection.id);
		expect(await catalog.documents.count()).toBe(1);
		expect(await catalog.memberships.count()).toBe(0);
		expect((await service.openDocument(document.id)).name).toBe('note.sdocx');
		await service.deleteDocuments([document.id]);
		expect(await catalog.documents.count()).toBe(0);
		expect(assets.files.size).toBe(0);
	});

	it('does not publish an original whose write failed and recovers partial assets', async () => {
		const { service, catalog, assets } = setup();
		vi.spyOn(assets, 'write').mockImplementationOnce(async (name) => {
			assets.files.set(name, new File(['partial'], name));
			throw new DOMException('Storage full', 'QuotaExceededError');
		});
		await expect(service.importDocument(note())).rejects.toThrow('Storage full');
		expect(await catalog.documents.count()).toBe(0);
		await service.recover();
		expect(assets.files.size).toBe(0);
	});

	it('recovers writes abandoned before the catalog commit without removing saved files', async () => {
		const { service, assets } = setup();
		await service.importDocument(note());
		await assets.write('originals/abandoned.sdocx', new Blob(['partial']));
		await expect(service.importDocument(note('other'), 'missing-collection')).rejects.toThrow();
		await service.recover();
		expect(assets.files.size).toBe(2);
	});

	it('retains pending deletions until failed asset cleanup succeeds', async () => {
		const { service, catalog, assets } = setup();
		const { document } = await service.importDocument(note());
		vi.spyOn(assets, 'remove').mockRejectedValueOnce(new Error('Busy'));
		await expect(service.deleteDocuments([document.id])).rejects.toThrow('Busy');
		expect(await catalog.documents.count()).toBe(0);
		expect(await catalog.pendingDeletes.count()).toBe(2);
		await service.recover();
		expect(await catalog.pendingDeletes.count()).toBe(0);
		expect(assets.files.size).toBe(0);
	});

	it('keeps a valid original when thumbnail storage fails', async () => {
		const { service, assets } = setup();
		const write = assets.write.bind(assets);
		vi.spyOn(assets, 'write').mockImplementation(async (name, blob) => {
			if (name.startsWith('thumbnails/')) throw new Error('Storage full');
			await write(name, blob);
		});
		const { document } = await service.importDocument(note());
		expect(document.thumbnail).toBeNull();
		expect(assets.files.size).toBe(1);
	});
});
