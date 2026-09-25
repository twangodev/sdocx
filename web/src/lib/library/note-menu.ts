import { separator, type MenuLeaf } from '$lib/menu';
import type { LibraryDocument } from './model';

export type NoteAction = 'open' | 'favorite' | 'download' | 'delete';

export function noteMenu(document: LibraryDocument): MenuLeaf<NoteAction>[] {
	return [
		{ kind: 'action', label: 'Open note', action: 'open' },
		{ kind: 'action', label: document.favorite ? 'Unfavorite' : 'Favorite', action: 'favorite' },
		{ kind: 'action', label: 'Download original', action: 'download' },
		separator(),
		{ kind: 'action', label: 'Delete from library', action: 'delete' }
	];
}

export function noteTitle(document: LibraryDocument): string {
	return document.title === document.filename
		? document.filename.replace(/\.sdocx$/i, '')
		: document.title;
}
