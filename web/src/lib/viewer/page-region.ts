export interface Rectangle {
	left: number;
	top: number;
	width: number;
	height: number;
}

/** Backing-pixel coordinates relative to the page at this raster scale. */
export interface PageRegion {
	x: number;
	y: number;
	width: number;
	height: number;
	scale: number;
	pixelRatio: number;
}

export const MAX_VIEW_PIXELS = 8 * 1024 * 1024;

export function pageRegion(
	page: Rectangle,
	clip: Rectangle,
	width: number,
	height: number,
	pixelRatio: number
): PageRegion | null {
	if (page.width <= 0 || page.height <= 0 || width <= 0 || height <= 0)
		return null;
	const left = Math.max(page.left, clip.left),
		top = Math.max(page.top, clip.top);
	const right = Math.min(page.left + page.width, clip.left + clip.width);
	const bottom = Math.min(page.top + page.height, clip.top + clip.height);
	if (right <= left || bottom <= top) return null;
	const sx = width / page.width,
		sy = height / page.height;
	const visibleWidth = (right - left) * sx,
		visibleHeight = (bottom - top) * sy;
	// Reuse a resolution level while panning and during small zoom changes.
	const requested = 2 ** (Math.ceil(Math.log2(pixelRatio / sx) * 2) / 2);
	const scale = Math.min(
		requested,
		4094 / visibleWidth,
		4094 / visibleHeight,
		Math.sqrt((MAX_VIEW_PIXELS - 16384) / (visibleWidth * visibleHeight))
	);
	const x = Math.floor((left - page.left) * sx * scale);
	const y = Math.floor((top - page.top) * sy * scale);
	return {
		x,
		y,
		width: Math.ceil((right - page.left) * sx * scale) - x,
		height: Math.ceil((bottom - page.top) * sy * scale) - y,
		scale,
		pixelRatio
	};
}
