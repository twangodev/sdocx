import { expect, it, vi } from 'vitest';
import { ReplayRaster } from './replay-raster';
import type { ReplayStroke } from './model';

it('replays native dots at original sample boundaries, including skipped samples and reverse seeks', () => {
	const item: ReplayStroke = {
		offset: 1,
		milliseconds: true,
		stroke: {
			points: [{ x: 100, y: 100 }, { x: 110, y: 110 }, { x: 120, y: 120 }, { x: 130, y: 130 }],
			pressures: [], timestamps: [0, 10, 20, 30], tilts: [], orientations: [],
			pen_width: 9, color: null,
			bbox: { x_min: 100, y_min: 100, x_max: 130, y_max: 130 }
		},
		geometry: {
			points: [{ x: 100, y: 100 }, { x: 105, y: 103 }, { x: 130, y: 130 }],
			sample_ends: [0, 2, 2, 3], dot_radii: [1, 2, 3],
			width: 9, segment_widths: null, bounds: null, color: '#111111', opacity: 1,
			profile: 'FountainPen', support: 'reconstructed'
		}
	};
	const raster = new ReplayRaster({ entry: 0, width: 200, height: 200, strokes: [item], objects: [] }, '#111111');
	const context = { beginPath: vi.fn(), moveTo: vi.fn(), arc: vi.fn(), fill: vi.fn() };
	const ctx = context as unknown as CanvasRenderingContext2D;
	for (const [sample, count] of [[0, 0], [1, 2], [2, 2], [3, 3], [1, 2]]) {
		vi.clearAllMocks();
		raster.drawStroke(ctx, 0, sample);
		expect(context.arc).toHaveBeenCalledTimes(count);
		expect(context.fill).toHaveBeenCalledTimes(count ? 1 : 0);
		if (count) expect(context.arc).toHaveBeenNthCalledWith(2, 105, 103, 2, 0, Math.PI * 2);
	}
	expect(item.stroke.points).toHaveLength(4);
});
