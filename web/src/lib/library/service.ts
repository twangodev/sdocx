import { OpfsAssetStore, type AssetStore } from './asset-store';
import { LibraryCatalog } from './catalog';
import type { Collection, LibraryDocument, PreparedDocument } from './model';

export type ExclusiveAccess = <T>(action: () => Promise<T>) => Promise<T>;

export class LibraryService {
	constructor(
		readonly catalog = new LibraryCatalog(),
		readonly assets: AssetStore = new OpfsAssetStore(),
		private readonly exclusive: ExclusiveAccess = async (action) =>
			await navigator.locks.request('sdocx-library', action),
		private readonly changed: () => void = () => {}
	) {}

	private async mutate<T>(action: () => Promise<T>): Promise<T> {
		try {
			return await this.exclusive(action);
		} finally {
			this.changed();
		}
	}

	async importDocument(
		prepared: PreparedDocument,
		collectionId?: string
	): Promise<{ document: LibraryDocument; duplicate: boolean }> {
		return this.mutate(async () => {
			const existing = await this.catalog.documents
				.where('contentHash')
				.equals(prepared.contentHash)
				.first();
			if (existing) {
				await this.addMemberships([existing.id], collectionId);
				return { document: existing, duplicate: true };
			}
			const id = crypto.randomUUID();
			const original = `originals/${id}.sdocx`;
			let thumbnail: string | null = null;
			await this.assets.write(original, prepared.file);
			if (prepared.thumbnail) {
				const name = `thumbnails/${id}.png`;
				try {
					await this.assets.write(name, prepared.thumbnail);
					thumbnail = name;
				} catch {
					await this.assets.remove(name).catch(() => {});
				}
			}
			const document: LibraryDocument = {
				id,
				original,
				thumbnail,
				contentHash: prepared.contentHash,
				filename: prepared.file.name,
				title: prepared.title || prepared.file.name,
				size: prepared.file.size,
				pageCount: prepared.pageCount,
				importedAt: Date.now(),
				favorite: false
			};
			// OPFS and IndexedDB cannot share a transaction; recovery removes unreferenced assets.
			await this.catalog.transaction(
				'rw',
				[this.catalog.documents, this.catalog.memberships, this.catalog.collections],
				async () => {
					await this.catalog.documents.add(document);
					await this.addMemberships([id], collectionId);
				}
			);
			return { document, duplicate: false };
		});
	}

	async openDocument(id: string): Promise<File> {
		return this.exclusive(async () => {
			const document = await this.catalog.documents.get(id);
			if (!document) throw new Error('This note is no longer in the library.');
			const contents = await this.assets.read(document.original);
			return new File([contents], document.filename, { type: 'application/zip' });
		});
	}

	async saveCollection(name: string, id: string = crypto.randomUUID()): Promise<Collection> {
		const collection = { id, name: name.trim() };
		if (!collection.name) throw new Error('Enter a collection name.');
		return this.mutate(async () => {
			await this.catalog.collections.put(collection);
			return collection;
		});
	}

	async deleteCollection(id: string): Promise<void> {
		await this.mutate(() =>
			this.catalog.transaction(
				'rw',
				[this.catalog.collections, this.catalog.memberships],
				async () => {
					await this.catalog.memberships.where('collectionId').equals(id).delete();
					await this.catalog.collections.delete(id);
				}
			)
		);
	}

	private async addMemberships(ids: string[], collectionId?: string): Promise<void> {
		if (!collectionId) return;
		if (!(await this.catalog.collections.get(collectionId)))
			throw new Error('This collection no longer exists.');
		const documents = await this.catalog.documents.bulkGet(ids);
		await this.catalog.memberships.bulkPut(
			documents.flatMap((document) => (document ? [{ documentId: document.id, collectionId }] : []))
		);
	}

	async setMembership(ids: string[], collectionId: string, included: boolean): Promise<void> {
		await this.mutate(() =>
			this.catalog.transaction(
				'rw',
				[this.catalog.memberships, this.catalog.documents, this.catalog.collections],
				async () => {
					if (included) await this.addMemberships(ids, collectionId);
					else await this.catalog.memberships.bulkDelete(ids.map((id) => [collectionId, id]));
				}
			)
		);
	}

	async setFavorite(ids: string[], favorite: boolean): Promise<void> {
		await this.mutate(() =>
			this.catalog.transaction('rw', this.catalog.documents, async () => {
				for (const id of ids) await this.catalog.documents.update(id, { favorite });
			})
		);
	}

	async deleteDocuments(ids: string[]): Promise<void> {
		await this.mutate(async () => {
			await this.catalog.transaction(
				'rw',
				[this.catalog.documents, this.catalog.memberships, this.catalog.pendingDeletes],
				async () => {
					for (const id of ids) {
						const document = await this.catalog.documents.get(id);
						if (!document) continue;
						await this.catalog.pendingDeletes.bulkPut(
							[document.original, document.thumbnail]
								.filter((name): name is string => !!name)
								.map((name) => ({ name }))
						);
						await this.catalog.memberships.where('documentId').equals(id).delete();
						await this.catalog.documents.delete(id);
					}
				}
			);
			await this.removePendingAssets();
		});
	}

	private async removePendingAssets(): Promise<void> {
		for (const { name } of await this.catalog.pendingDeletes.toArray()) {
			await this.assets.remove(name);
			await this.catalog.pendingDeletes.delete(name);
		}
	}

	async cacheThumbnail(id: string, contents: Blob): Promise<void> {
		await this.mutate(async () => {
			const document = await this.catalog.documents.get(id);
			if (!document || document.thumbnail) return;
			await this.removePendingAssets();
			const thumbnail = `thumbnails/${id}.png`;
			await this.assets.write(thumbnail, contents);
			await this.catalog.documents.update(id, { thumbnail });
		});
	}

	async clearThumbnails(): Promise<void> {
		await this.mutate(async () => {
			const names = (await this.assets.list()).filter((name) => name.startsWith('thumbnails/'));
			await this.catalog.transaction(
				'rw',
				[this.catalog.documents, this.catalog.pendingDeletes],
				async () => {
					await this.catalog.pendingDeletes.bulkPut(names.map((name) => ({ name })));
					await this.catalog.documents.toCollection().modify({ thumbnail: null });
				}
			);
			await this.removePendingAssets();
		});
	}

	async clearLibrary(): Promise<void> {
		await this.mutate(async () => {
			const names = await this.assets.list();
			await this.catalog.transaction(
				'rw',
				[
					this.catalog.documents,
					this.catalog.collections,
					this.catalog.memberships,
					this.catalog.pendingDeletes
				],
				async () => {
					await this.catalog.pendingDeletes.bulkPut(names.map((name) => ({ name })));
					await this.catalog.documents.clear();
					await this.catalog.collections.clear();
					await this.catalog.memberships.clear();
				}
			);
			await this.removePendingAssets();
		});
	}

	async recover(): Promise<void> {
		await this.exclusive(async () => {
			await this.removePendingAssets();
			const documents = await this.catalog.documents.toArray();
			const referenced = new Set(
				documents.flatMap((document) => [document.original, document.thumbnail])
			);
			for (const name of await this.assets.list()) {
				if (!referenced.has(name)) await this.assets.remove(name);
			}
		});
	}
}
