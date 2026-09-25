import Dexie, { type Table } from 'dexie';
import type { Collection, LibraryDocument, LibrarySnapshot, Membership } from './model';

export class LibraryCatalog extends Dexie {
	documents!: Table<LibraryDocument, string>;
	collections!: Table<Collection, string>;
	memberships!: Table<Membership, [string, string]>;
	pendingDeletes!: Table<{ name: string }, string>;

	constructor(name = 'sdocx-library') {
		super(name);
		this.version(1).stores({
			documents: '&id, &contentHash, importedAt',
			collections: '&id',
			memberships: '[collectionId+documentId], collectionId, documentId',
			pendingDeletes: '&name'
		});
	}

	async snapshot(): Promise<LibrarySnapshot> {
		return this.transaction('r', [this.documents, this.collections, this.memberships], async () => ({
			documents: await this.documents.toArray(),
			collections: await this.collections.toArray(),
			memberships: await this.memberships.toArray()
		}));
	}
}
