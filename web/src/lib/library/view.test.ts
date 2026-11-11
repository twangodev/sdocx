import { expect, it } from 'vitest';
import { libraryCounts, visibleDocuments } from './view';
import type { LibraryDocument, LibrarySnapshot } from './model';

const now = Date.now();
const documents = [
	{ id: 'a', title: 'Biology', filename: 'lecture.sdocx', importedAt: now, favorite: true },
	{
		id: 'b',
		title: 'Algebra',
		filename: 'math.sdocx',
		importedAt: now - 31 * 86400000,
		favorite: false
	}
] as LibraryDocument[];
const snapshot: LibrarySnapshot = {
	documents,
	collections: [{ id: 'course', name: 'Course' }],
	memberships: [{ collectionId: 'course', documentId: 'b' }]
};
it('combines source membership with case-insensitive title and filename search', () => {
	expect(visibleDocuments(snapshot, 'all', 'LECTURE', 'title', now).map((note) => note.id)).toEqual(
		['a']
	);
	expect(
		visibleDocuments(snapshot, { collectionId: 'course' }, '', 'newest', now).map((note) => note.id)
	).toEqual(['b']);
	expect(visibleDocuments(snapshot, 'favorites', '', 'newest', now).map((note) => note.id)).toEqual(
		['a']
	);
	expect(visibleDocuments(snapshot, 'recent', '', 'newest', now).map((note) => note.id)).toEqual([
		'a'
	]);
	expect(visibleDocuments(snapshot, 'all', '', 'title', now).map((note) => note.id)).toEqual([
		'b',
		'a'
	]);
});

it('counts source documents with the same inclusive Recent boundary and invalid timestamp behavior', () => {
	const cutoff = now - 30 * 86400000;
	const importedAt = [cutoff - 1, cutoff, cutoff + 1, now + 86400000, NaN];
	const notes: LibrarySnapshot = {
		documents: importedAt.map((timestamp, index) => ({
			...documents[0],
			id: String(index),
			importedAt: timestamp,
			favorite: index % 2 === 0
		})),
		collections: [],
		memberships: []
	};
	const counts = libraryCounts(notes, now);
	expect(counts.all).toBe(5);
	expect(counts.recent).toBe(4);
	expect(counts.favorites).toBe(3);
	for (const source of ['all', 'recent', 'favorites'] as const) {
		expect(counts[source]).toBe(visibleDocuments(notes, source, '', 'newest', now).length);
	}
	expect(libraryCounts(notes, NaN).recent).toBe(5);
	expect(visibleDocuments(notes, 'recent', '', 'newest', NaN)).toHaveLength(5);
});

it('counts collection membership rows without deduplicating or excluding missing notes', () => {
	const notes: LibrarySnapshot = {
		...snapshot,
		collections: [...snapshot.collections, { id: 'empty', name: 'Empty' }],
		memberships: [
			{ collectionId: 'course', documentId: 'a' },
			{ collectionId: 'course', documentId: 'a' },
			{ collectionId: 'course', documentId: 'missing-note' },
			{ collectionId: 'missing-collection', documentId: 'b' }
		]
	};
	const { collectionMemberships } = libraryCounts(notes, now);
	expect(collectionMemberships.get('course')).toBe(3);
	expect(collectionMemberships.get('missing-collection')).toBe(1);
	expect(collectionMemberships.get('empty') ?? 0).toBe(0);
	for (const collection of notes.collections) {
		expect(collectionMemberships.get(collection.id) ?? 0).toBe(
			notes.memberships.filter((membership) => membership.collectionId === collection.id).length
		);
	}
});
