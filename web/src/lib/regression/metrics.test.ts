import { describe, expect, it } from 'vitest';
import { comparePixels, compareSourceDimensions, type PixelBuffer } from './metrics';

function pixels(data: number[], width = 1, height = 1): PixelBuffer {
	return { width, height, data: Uint8ClampedArray.from(data) };
}

describe('visual comparison metrics', () => {
	it('reports no change for identical pixels', () => {
		const result = comparePixels(pixels([10, 20, 30, 255]), pixels([10, 20, 30, 255]));
		expect(result.metrics).toMatchObject({
			meanAbsoluteError: 0,
			rootMeanSquareError: 0,
			changedPixelRatio: 0,
			changedPixels: 0,
			totalPixels: 1
		});
		expect(Array.from(result.heatmap)).toEqual([0, 96, 0, 0]);
	});

	it('composites transparent pixels on white and applies the changed-pixel threshold', () => {
		const transparentBlack = pixels([0, 0, 0, 0]);
		const opaqueWhite = pixels([255, 255, 255, 255]);
		expect(comparePixels(transparentBlack, opaqueWhite).metrics.changedPixels).toBe(0);

		const result = comparePixels(pixels([0, 0, 0, 255]), opaqueWhite, 16);
		expect(result.metrics.meanAbsoluteError).toBe(1);
		expect(result.metrics.rootMeanSquareError).toBe(1);
		expect(result.metrics.changedPixelRatio).toBe(1);
		expect(Array.from(result.heatmap)).toEqual([255, 0, 0, 255]);
	});

	it('rejects incompatible pixel buffers', () => {
		expect(() => comparePixels(pixels([0, 0, 0, 0]), pixels([], 0, 0))).toThrow(/dimensions/);
		expect(() => comparePixels(pixels([0, 0, 0], 1, 1), pixels([0, 0, 0], 1, 1))).toThrow(
			/length/
		);
	});

	it('reports source dimensions and informational aspect-ratio mismatches', () => {
		expect(compareSourceDimensions(1200, 1600, 600, 800)).toMatchObject({
			generatedSourceWidth: 1200,
			referenceSourceHeight: 800,
			aspectRatioDelta: 0,
			aspectRatioMismatch: false
		});

		const mismatch = compareSourceDimensions(1200, 1600, 612, 792);
		expect(mismatch.aspectRatioDelta).toBeCloseTo(0.0294, 3);
		expect(mismatch.aspectRatioMismatch).toBe(true);
		expect(() => compareSourceDimensions(0, 1, 1, 1)).toThrow(/positive/);
	});
});
