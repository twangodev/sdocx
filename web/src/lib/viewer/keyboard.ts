export type ViewerKeyboardCommand =
	| 'zoom-in'
	| 'zoom-out'
	| 'previous-page'
	| 'next-page'
	| 'first-page'
	| 'last-page';

export interface ViewerKeyStroke {
	key: string;
	code?: string;
	ctrlKey: boolean;
	metaKey: boolean;
	altKey: boolean;
	shiftKey: boolean;
}

export function viewerCommandForKey(event: ViewerKeyStroke): ViewerKeyboardCommand | undefined {
	const primaryModifier = event.ctrlKey || event.metaKey;

	if (primaryModifier && !event.altKey) {
		if (event.key === '+' || event.key === '=' || event.code === 'NumpadAdd') return 'zoom-in';
		if (event.key === '-' || event.code === 'NumpadSubtract') return 'zoom-out';
		return undefined;
	}

	if (event.altKey || event.shiftKey) return undefined;

	switch (event.key) {
		case 'PageUp':
			return 'previous-page';
		case 'PageDown':
			return 'next-page';
		case 'Home':
			return 'first-page';
		case 'End':
			return 'last-page';
		default:
			return undefined;
	}
}
