export type LibraryDialogIntent =
	| { kind: 'create-collection'; collectionId: string; name: string; documentIds: string[] }
	| { kind: 'rename-collection'; collectionId: string; name: string }
	| { kind: 'delete-collection'; collectionId: string; name: string }
	| { kind: 'delete-notes'; documentIds: string[] };
