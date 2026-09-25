import { expect, it } from 'vitest';
import { visibleDocuments } from './view';
import type { LibraryDocument, LibrarySnapshot } from './model';

const now = Date.now();
const documents = [
	{ id: 'a', title: 'Biology', filename: 'lecture.sdocx', importedAt: now, favorite: true },
	{ id: 'b', title: 'Algebra', filename: 'math.sdocx', importedAt: now - 31 * 86400000, favorite: false }
] as LibraryDocument[];
const snapshot: LibrarySnapshot = { documents, collections: [{ id: 'course', name: 'Course' }], memberships: [{ collectionId: 'course', documentId: 'b' }] };
it('combines source membership with case-insensitive title and filename search', () => {
	expect(visibleDocuments(snapshot, 'all', 'LECTURE', 'title', now).map((note) => note.id)).toEqual(['a']);
	expect(visibleDocuments(snapshot, { collectionId: 'course' }, '', 'newest', now).map((note) => note.id)).toEqual(['b']);
	expect(visibleDocuments(snapshot, 'favorites', '', 'newest', now).map((note) => note.id)).toEqual(['a']);
	expect(visibleDocuments(snapshot, 'recent', '', 'newest', now).map((note) => note.id)).toEqual(['a']);
	expect(visibleDocuments(snapshot, 'all', '', 'title', now).map((note) => note.id)).toEqual(['b', 'a']);
});
