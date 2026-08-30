export interface PixelBuffer {
	width: number;
	height: number;
	data: Uint8ClampedArray;
}

export interface VisualMetrics {
	width: number;
	height: number;
	meanAbsoluteError: number;
	rootMeanSquareError: number;
	changedPixelRatio: number;
	changedPixels: number;
	totalPixels: number;
	threshold: number;
}

export interface SourceDimensionMetrics {
	generatedSourceWidth: number;
	generatedSourceHeight: number;
	referenceSourceWidth: number;
	referenceSourceHeight: number;
	generatedAspectRatio: number;
	referenceAspectRatio: number;
	aspectRatioDelta: number;
	aspectRatioMismatch: boolean;
}

export type PageVisualMetrics = VisualMetrics & SourceDimensionMetrics;

export interface PixelComparison {
	metrics: VisualMetrics;
	heatmap: Uint8ClampedArray;
}

function composite(channel: number, alpha: number): number {
	return Math.round((channel * alpha + 255 * (255 - alpha)) / 255);
}

export function compareSourceDimensions(
	generatedWidth: number,
	generatedHeight: number,
	referenceWidth: number,
	referenceHeight: number,
	mismatchThreshold = 0.001
): SourceDimensionMetrics {
	if (
		![generatedWidth, generatedHeight, referenceWidth, referenceHeight].every(
			(value) => Number.isFinite(value) && value > 0
		)
	) {
		throw new Error('Source dimensions must be positive finite values.');
	}
	const generatedAspectRatio = generatedWidth / generatedHeight;
	const referenceAspectRatio = referenceWidth / referenceHeight;
	const aspectRatioDelta = Math.abs(generatedAspectRatio - referenceAspectRatio) / referenceAspectRatio;
	return {
		generatedSourceWidth: generatedWidth,
		generatedSourceHeight: generatedHeight,
		referenceSourceWidth: referenceWidth,
		referenceSourceHeight: referenceHeight,
		generatedAspectRatio,
		referenceAspectRatio,
		aspectRatioDelta,
		aspectRatioMismatch: aspectRatioDelta > mismatchThreshold
	};
}

export function comparePixels(
	actual: PixelBuffer,
	reference: PixelBuffer,
	threshold = 16
): PixelComparison {
	if (actual.width !== reference.width || actual.height !== reference.height) {
		throw new Error('Pixel buffers must have identical dimensions.');
	}
	if (actual.data.length !== reference.data.length || actual.data.length !== actual.width * actual.height * 4) {
		throw new Error('Pixel buffer length does not match its dimensions.');
	}

	const heatmap = new Uint8ClampedArray(actual.data.length);
	const totalPixels = actual.width * actual.height;
	let absoluteError = 0;
	let squaredError = 0;
	let changedPixels = 0;

	for (let offset = 0; offset < actual.data.length; offset += 4) {
		const actualAlpha = actual.data[offset + 3];
		const referenceAlpha = reference.data[offset + 3];
		let maximum = 0;
		for (let channel = 0; channel < 3; channel += 1) {
			const actualValue = composite(actual.data[offset + channel], actualAlpha);
			const referenceValue = composite(reference.data[offset + channel], referenceAlpha);
			const difference = Math.abs(actualValue - referenceValue);
			maximum = Math.max(maximum, difference);
			absoluteError += difference;
			squaredError += difference * difference;
		}

		if (maximum > threshold) changedPixels += 1;
		heatmap[offset] = maximum;
		heatmap[offset + 1] = Math.max(0, 96 - maximum);
		heatmap[offset + 2] = 0;
		heatmap[offset + 3] = maximum === 0 ? 0 : Math.max(72, maximum);
	}

	const channelCount = totalPixels * 3;
	return {
		metrics: {
			width: actual.width,
			height: actual.height,
			meanAbsoluteError: absoluteError / channelCount / 255,
			rootMeanSquareError: Math.sqrt(squaredError / channelCount) / 255,
			changedPixelRatio: changedPixels / totalPixels,
			changedPixels,
			totalPixels,
			threshold
		},
		heatmap
	};
}
