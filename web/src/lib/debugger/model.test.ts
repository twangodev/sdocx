import { describe, it, expect } from 'vitest';
import { timeline, sampleAt, hexRows, type ReplayStroke } from './model';
function stroke(
	timestamps: number[],
	milliseconds = true,
	points = timestamps.length
): ReplayStroke {
	return {
		offset: 17,
		paint: { color: '#1a1a1a', width: 1, segment_widths: null },
		milliseconds,
		stroke: {
			points: Array.from({ length: points }, (_, i) => ({ x: i, y: i })),
			timestamps,
			pressures: [],
			tilts: [],
			orientations: [],
			pen_width: 2,
			color: null,
			bbox: { x_min: 0, y_min: 0, x_max: 10, y_max: 10 }
		}
	};
}
describe('stored stroke timeline', () => {
	it('uses relative times within each stroke and a synthetic gap between strokes', () => {
		const tracks = timeline([stroke([60, 70, 90]), stroke([0, 25])]);
		expect(tracks[0]).toMatchObject({
			start: 0,
			end: 30,
			times: [0, 10, 30],
			synthetic: false
		});
		expect(tracks[1]).toMatchObject({ start: 180, end: 205 });
	});
	it('retains equal timestamps and uses the last sample at that instant', () => {
		expect(sampleAt([0, 0, 12], 0)).toBe(1);
		expect(sampleAt([0, 0, 12], -1)).toBe(-1);
	});
	it('uses synthetic timing for unknown units, decreasing or incomplete channels', () => {
		for (const s of [
			stroke([0, 2], false),
			stroke([4, 2]),
			stroke([0], true, 2)
		])
			expect(timeline([s])[0]).toMatchObject({
				times: [0, 16],
				synthetic: true
			});
	});
	it('handles empty pages and zero/single point strokes', () => {
		expect(timeline([])).toEqual([]);
		expect(timeline([stroke([])])[0].end).toBe(0);
		expect(timeline([stroke([99])])[0].times).toEqual([0]);
		expect(sampleAt([], 10)).toBe(-1);
	});
	it('shows absolute hex offsets and printable ASCII without interpreting markup', () => {
		expect(hexRows([60, 65, 0, 255], 4096)).toContain('00001000  3c 41 00 ff');
		expect(hexRows([60, 65, 0, 255], 4096)).toContain('<A..');
	});
});
