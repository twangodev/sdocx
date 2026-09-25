export interface LibraryDocument {
	id: string;
	contentHash: string;
	filename: string;
	title: string;
	size: number;
	pageCount: number;
	importedAt: number;
	favorite: boolean;
	original: string;
	thumbnail: string | null;
}

export interface Collection {
	id: string;
	name: string;
}

export interface Membership {
	collectionId: string;
	documentId: string;
}

export interface LibrarySnapshot {
	documents: LibraryDocument[];
	collections: Collection[];
	memberships: Membership[];
}

export interface PreparedDocument {
	file: File;
	contentHash: string;
	title: string;
	pageCount: number;
	thumbnail: Blob | null;
}

export type LibrarySource = 'all' | 'recent' | 'favorites' | { collectionId: string };
export type LibrarySort = 'newest' | 'title';

export function errorMessage(error: unknown): string {
	return error instanceof Error ? error.message : 'Unable to complete this action.';
}
