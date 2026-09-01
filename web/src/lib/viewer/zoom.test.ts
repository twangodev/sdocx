import { describe, expect, it } from 'vitest';
import { clampZoom, formatZoom, nextZoomStep, roundedZoom } from './zoom';

describe('document zoom', () => {
	it('clamps and rounds continuous zoom values', () => {
		expect(roundedZoom(108.349)).toBe(108.3);
		expect(clampZoom(2)).toBe(10);
		expect(clampZoom(500)).toBe(400);
	});

	it('steps from continuous values to the next preset', () => {
		expect(nextZoomStep(100, 1)).toBe(125);
		expect(nextZoomStep(108.3, 1)).toBe(125);
		expect(nextZoomStep(108.3, -1)).toBe(100);
		expect(nextZoomStep(20, -1)).toBe(10);
	});

	it('formats whole and fractional percentages compactly', () => {
		expect(formatZoom(125)).toBe('125%');
		expect(formatZoom(108.3)).toBe('108.3%');
	});
});
