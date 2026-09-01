import { describe, expect, it } from 'vitest';
import { createWheelZoomAccumulator, type WheelZoomSample } from './wheel-zoom';

function sample(deltaY: number, timeStamp: number, overrides: Partial<WheelZoomSample> = {}) {
	return {
		deltaY,
		deltaMode: 0,
		viewportHeight: 800,
		timeStamp,
		...overrides
	};
}

describe('trackpad zoom accumulation', () => {
	it('turns a deliberate pinch into one zoom step', () => {
		const accumulator = createWheelZoomAccumulator();
		expect(accumulator.push(sample(-8, 0))).toBeUndefined();
		expect(accumulator.push(sample(-8, 10))).toBeUndefined();
		expect(accumulator.push(sample(-8, 20))).toBe(1);
	});

	it('maps positive deltas to zooming out', () => {
		const accumulator = createWheelZoomAccumulator();
		expect(accumulator.push(sample(100, 0))).toBe(-1);
	});

	it('resets accumulation when direction changes or a gesture pauses', () => {
		const accumulator = createWheelZoomAccumulator();
		expect(accumulator.push(sample(-16, 0))).toBeUndefined();
		expect(accumulator.push(sample(16, 10))).toBeUndefined();
		expect(accumulator.push(sample(16, 20))).toBe(-1);
		expect(accumulator.push(sample(-16, 30))).toBeUndefined();
		expect(accumulator.push(sample(-16, 300))).toBeUndefined();
	});

	it('normalizes line-based wheel input', () => {
		const accumulator = createWheelZoomAccumulator();
		expect(accumulator.push(sample(-2, 0, { deltaMode: 1 }))).toBe(1);
	});
});
