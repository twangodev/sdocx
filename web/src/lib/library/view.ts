import type { LibraryDocument, LibrarySnapshot, LibrarySort, LibrarySource } from './model';

export function visibleDocuments(snapshot: LibrarySnapshot, source: LibrarySource, search: string, sort: LibrarySort, now = Date.now()): LibraryDocument[] {
	const members = typeof source === 'object'
		? new Set(snapshot.memberships.filter((item) => item.collectionId === source.collectionId).map((item) => item.documentId))
		: null;
	const query = search.trim().toLocaleLowerCase();
	return snapshot.documents.filter((document) => {
		if (members && !members.has(document.id)) return false;
		if (source === 'favorites' && !document.favorite) return false;
		if (source === 'recent' && document.importedAt < now - 30 * 24 * 60 * 60 * 1000) return false;
		return `${document.title}\n${document.filename}`.toLocaleLowerCase().includes(query);
	}).sort((a, b) => sort === 'title' ? a.title.localeCompare(b.title) || a.id.localeCompare(b.id) : b.importedAt - a.importedAt || a.id.localeCompare(b.id));
}
