import { describe, expect, it } from 'vitest';
import { pageRegion, MAX_VIEW_PIXELS } from './page-region';

describe('visible page raster geometry', () => {
	it('renders a zoomed long page at screen density without allocating the whole page', () => {
		const result = pageRegion(
			{ left: -600, top: -12000, width: 4000, height: 80000 },
			{ left: 0, top: 0, width: 800, height: 600 },
			1000,
			20000,
			2
		)!;
		expect(result.scale).toBe(8);
		expect(result).toMatchObject({
			x: 1200,
			y: 24000,
			width: 1600,
			height: 1200
		});
	});
	it('keeps the same resolution level while panning and maps the clip to source coordinates', () => {
		const clip = { left: 20, top: 30, width: 800, height: 600 };
		const first = pageRegion(
			{ left: 0, top: 0, width: 1200, height: 2400 },
			clip,
			1000,
			2000,
			2
		)!;
		const second = pageRegion(
			{ left: 0, top: -500, width: 1200, height: 2400 },
			clip,
			1000,
			2000,
			2
		)!;
		expect(second.scale).toBe(first.scale);
		expect((second.y - first.y) / first.scale).toBeCloseTo(500 / 1.2, 0);
		expect(first.scale).toBeGreaterThanOrEqual(2.4);
	});
	it('clips offscreen pages and caps oversized backing stores', () => {
		expect(
			pageRegion(
				{ left: 900, top: 0, width: 800, height: 1000 },
				{ left: 0, top: 0, width: 800, height: 600 },
				800,
				1000,
				1
			)
		).toBeNull();
		const region = pageRegion(
			{ left: 0, top: 0, width: 10000, height: 20000 },
			{ left: 0, top: 0, width: 8000, height: 5000 },
			1000,
			2000,
			4
		)!;
		expect(region.width * region.height).toBeLessThanOrEqual(MAX_VIEW_PIXELS);
		expect(Math.max(region.width, region.height)).toBeLessThanOrEqual(4096);
	});
});
