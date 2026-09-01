export type ZoomDirection = -1 | 1;

export interface ZoomAnchor {
	clientX: number;
	clientY: number;
}

export interface WheelZoomSample {
	deltaY: number;
	deltaMode: number;
	viewportHeight: number;
	timeStamp: number;
}

export interface WheelZoomOptions {
	disabled?: boolean;
	onZoom: (direction: ZoomDirection, anchor: ZoomAnchor) => void;
}

const PIXEL_THRESHOLD = 24;
const GESTURE_GAP_MS = 180;

function pixelDelta(sample: WheelZoomSample): number {
	if (sample.deltaMode === 1) return sample.deltaY * 16;
	if (sample.deltaMode === 2) {
		return sample.deltaY * sample.viewportHeight;
	}
	return sample.deltaY;
}

export function createWheelZoomAccumulator() {
	let accumulated = 0;
	let lastTime = 0;
	let lastDirection = 0;

	return {
		push(sample: WheelZoomSample): ZoomDirection | undefined {
			const delta = pixelDelta(sample);
			const direction = Math.sign(delta);
			if (direction === 0) return undefined;

			if (sample.timeStamp - lastTime > GESTURE_GAP_MS || direction !== lastDirection) {
				accumulated = 0;
			}

			lastTime = sample.timeStamp;
			lastDirection = direction;
			accumulated += delta;

			if (Math.abs(accumulated) < PIXEL_THRESHOLD) return undefined;
			accumulated = 0;
			return direction < 0 ? 1 : -1;
		}
	};
}

export function wheelZoom(node: HTMLElement, initialOptions: WheelZoomOptions) {
	let options = initialOptions;
	const accumulator = createWheelZoomAccumulator();

	function handleWheel(event: WheelEvent): void {
		if (!event.ctrlKey) return;
		if (event.cancelable) event.preventDefault();
		if (options.disabled) return;

		const direction = accumulator.push({
			deltaY: event.deltaY,
			deltaMode: event.deltaMode,
			viewportHeight: node.clientHeight,
			timeStamp: event.timeStamp
		});
		if (direction) options.onZoom(direction, { clientX: event.clientX, clientY: event.clientY });
	}

	node.addEventListener('wheel', handleWheel, { passive: false });

	return {
		update(nextOptions: WheelZoomOptions) {
			options = nextOptions;
		},
		destroy() {
			node.removeEventListener('wheel', handleWheel);
		}
	};
}
