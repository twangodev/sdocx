import { describe, expect, it } from 'vitest';
import { pixelDelta, wheelZoomFactor, type WheelZoomSample } from './wheel-zoom';

function sample(deltaY: number, timeStamp: number, overrides: Partial<WheelZoomSample> = {}) {
	return {
		deltaY,
		deltaMode: 0,
		viewportHeight: 800,
		timeStamp,
		...overrides
	};
}

describe('trackpad zoom', () => {
	it('turns every pinch delta into a proportional scale', () => {
		expect(wheelZoomFactor(sample(-8, 0))).toBeCloseTo(1.066, 3);
		expect(wheelZoomFactor(sample(8, 10))).toBeCloseTo(0.938, 3);
	});

	it('composes continuous deltas without quantizing them into steps', () => {
		const combined = wheelZoomFactor(sample(-4, 0)) * wheelZoomFactor(sample(-4, 10));
		expect(combined).toBeCloseTo(wheelZoomFactor(sample(-8, 20)), 8);
	});

	it('normalizes line and page-based wheel input', () => {
		expect(pixelDelta(sample(-2, 0, { deltaMode: 1 }))).toBe(-32);
		expect(pixelDelta(sample(1, 0, { deltaMode: 2 }))).toBe(800);
	});
});
