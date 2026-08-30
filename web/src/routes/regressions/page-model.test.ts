import { describe, expect, it } from 'vitest';
import { clampPageIndex, statusTone } from './page-model';

describe('regression page model', () => {
	it('maps terminal and active fixture states to presentation tones', () => {
		expect(statusTone('queued')).toBe('neutral');
		expect(statusTone('downloading')).toBe('active');
		expect(statusTone('passed')).toBe('success');
		expect(statusTone('failed')).toBe('danger');
		expect(statusTone('cancelled')).toBe('danger');
	});

	it('keeps the selected page within the available comparison pages', () => {
		expect(clampPageIndex(2.9, 5)).toBe(2);
		expect(clampPageIndex(-1, 5)).toBe(0);
		expect(clampPageIndex(8, 5)).toBe(4);
		expect(clampPageIndex(3, 0)).toBe(0);
	});
});
