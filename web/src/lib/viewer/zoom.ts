export type ZoomDirection = -1 | 1;

export const MIN_ZOOM = 10;
export const MAX_ZOOM = 400;
export const ZOOM_STEPS = [25, 50, 75, 100, 125, 150, 175, 200, 300, 400] as const;

export function clampZoom(zoom: number): number {
	return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom));
}

export function roundedZoom(zoom: number): number {
	return Math.round(clampZoom(zoom) * 10) / 10;
}

export function nextZoomStep(zoom: number, direction: ZoomDirection): number {
	if (direction > 0) {
		return ZOOM_STEPS.find((step) => step > zoom) ?? MAX_ZOOM;
	}

	return ZOOM_STEPS.findLast((step) => step < zoom) ?? MIN_ZOOM;
}

export function formatZoom(zoom: number): string {
	return Number.isInteger(zoom) ? `${zoom}%` : `${zoom.toFixed(1)}%`;
}
