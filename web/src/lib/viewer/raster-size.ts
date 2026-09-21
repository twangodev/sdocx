// Backing-store limits for whole-page gesture previews.
// Zoom-aware Canvas layers use the visible-region limits in page-region.ts.
export function rasterSize(width: number, height: number) {
	const scale = Math.min(
		1,
		4096 / width,
		4096 / height,
		Math.sqrt((4 * 1024 * 1024) / (width * height))
	);
	return {
		width: Math.max(1, Math.floor(width * scale)),
		height: Math.max(1, Math.floor(height * scale)),
		scale
	};
}
