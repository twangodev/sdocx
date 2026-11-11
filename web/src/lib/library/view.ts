import type { LibraryDocument, LibrarySnapshot, LibrarySort, LibrarySource } from './model';

const RECENT_WINDOW_MS = 30 * 24 * 60 * 60 * 1000;

function isRecent(importedAt: number, now: number): boolean {
	return !(importedAt < now - RECENT_WINDOW_MS);
}

export function libraryCounts(snapshot: LibrarySnapshot, now = Date.now()) {
	let recent = 0;
	let favorites = 0;
	for (const document of snapshot.documents) {
		if (isRecent(document.importedAt, now)) recent += 1;
		if (document.favorite) favorites += 1;
	}
	const collectionMemberships = new Map<string, number>();
	for (const membership of snapshot.memberships) {
		const count = collectionMemberships.get(membership.collectionId) ?? 0;
		collectionMemberships.set(membership.collectionId, count + 1);
	}
	return { all: snapshot.documents.length, recent, favorites, collectionMemberships };
}

export function visibleDocuments(
	snapshot: LibrarySnapshot,
	source: LibrarySource,
	search: string,
	sort: LibrarySort,
	now = Date.now()
): LibraryDocument[] {
	const members =
		typeof source === 'object'
			? new Set(
					snapshot.memberships
						.filter((item) => item.collectionId === source.collectionId)
						.map((item) => item.documentId)
				)
			: null;
	const query = search.trim().toLocaleLowerCase();
	return snapshot.documents
		.filter((document) => {
			if (members && !members.has(document.id)) return false;
			if (source === 'favorites' && !document.favorite) return false;
			if (source === 'recent' && !isRecent(document.importedAt, now)) return false;
			return `${document.title}\n${document.filename}`.toLocaleLowerCase().includes(query);
		})
		.sort((a, b) =>
			sort === 'title'
				? a.title.localeCompare(b.title) || a.id.localeCompare(b.id)
				: b.importedAt - a.importedAt || a.id.localeCompare(b.id)
		);
}
