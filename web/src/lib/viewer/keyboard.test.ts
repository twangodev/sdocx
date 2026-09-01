import { describe, expect, it } from 'vitest';
import { viewerCommandForKey, type ViewerKeyStroke } from './keyboard';

function stroke(key: string, overrides: Partial<ViewerKeyStroke> = {}): ViewerKeyStroke {
	return {
		key,
		code: '',
		ctrlKey: false,
		metaKey: false,
		altKey: false,
		shiftKey: false,
		...overrides
	};
}

describe('viewer keyboard shortcuts', () => {
	it('maps control and command zoom shortcuts', () => {
		expect(viewerCommandForKey(stroke('=', { ctrlKey: true }))).toBe('zoom-in');
		expect(viewerCommandForKey(stroke('+', { metaKey: true, shiftKey: true }))).toBe('zoom-in');
		expect(viewerCommandForKey(stroke('-', { ctrlKey: true }))).toBe('zoom-out');
		expect(viewerCommandForKey(stroke('Add', { ctrlKey: true, code: 'NumpadAdd' }))).toBe(
			'zoom-in'
		);
		expect(viewerCommandForKey(stroke('Subtract', { metaKey: true, code: 'NumpadSubtract' }))).toBe(
			'zoom-out'
		);
	});

	it('maps document navigation keys', () => {
		expect(viewerCommandForKey(stroke('PageUp'))).toBe('previous-page');
		expect(viewerCommandForKey(stroke('PageDown'))).toBe('next-page');
		expect(viewerCommandForKey(stroke('Home'))).toBe('first-page');
		expect(viewerCommandForKey(stroke('End'))).toBe('last-page');
	});

	it('leaves browser and modified navigation shortcuts alone', () => {
		expect(viewerCommandForKey(stroke('0', { ctrlKey: true }))).toBeUndefined();
		expect(viewerCommandForKey(stroke('PageDown', { shiftKey: true }))).toBeUndefined();
		expect(viewerCommandForKey(stroke('+', { altKey: true }))).toBeUndefined();
		expect(viewerCommandForKey(stroke('ArrowRight'))).toBeUndefined();
	});
});
