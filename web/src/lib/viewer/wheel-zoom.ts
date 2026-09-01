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
	onZoom: (factor: number, anchor: ZoomAnchor) => void;
	onEnd?: () => void;
}

const GESTURE_GAP_MS = 140;
const TRACKPAD_SENSITIVITY = 0.008;

export function pixelDelta(sample: WheelZoomSample): number {
	if (sample.deltaMode === 1) return sample.deltaY * 16;
	if (sample.deltaMode === 2) {
		return sample.deltaY * sample.viewportHeight;
	}
	return sample.deltaY;
}

export function wheelZoomFactor(sample: WheelZoomSample): number {
	return Math.exp(-pixelDelta(sample) * TRACKPAD_SENSITIVITY);
}

export function wheelZoom(node: HTMLElement, initialOptions: WheelZoomOptions) {
	let options = initialOptions;
	let pendingFactor = 1;
	let pendingAnchor: ZoomAnchor | undefined;
	let frame = 0;
	let endTimer: ReturnType<typeof setTimeout> | undefined;

	function cancelPending(): void {
		if (frame) cancelAnimationFrame(frame);
		if (endTimer) clearTimeout(endTimer);
		frame = 0;
		endTimer = undefined;
		pendingFactor = 1;
		pendingAnchor = undefined;
	}

	function flush(): void {
		frame = 0;
		if (!pendingAnchor || pendingFactor === 1 || options.disabled) return;
		const factor = pendingFactor;
		const anchor = pendingAnchor;
		pendingFactor = 1;
		pendingAnchor = undefined;
		options.onZoom(factor, anchor);
	}

	function handleWheel(event: WheelEvent): void {
		if (!event.ctrlKey) return;
		if (event.cancelable) event.preventDefault();
		if (options.disabled) return;

		pendingFactor *= wheelZoomFactor({
			deltaY: event.deltaY,
			deltaMode: event.deltaMode,
			viewportHeight: node.clientHeight,
			timeStamp: event.timeStamp
		});
		pendingAnchor = { clientX: event.clientX, clientY: event.clientY };
		if (!frame) frame = requestAnimationFrame(flush);

		if (endTimer) clearTimeout(endTimer);
		endTimer = setTimeout(() => {
			if (frame) {
				cancelAnimationFrame(frame);
				flush();
			}
			endTimer = undefined;
			options.onEnd?.();
		}, GESTURE_GAP_MS);
	}

	node.addEventListener('wheel', handleWheel, { passive: false });

	return {
		update(nextOptions: WheelZoomOptions) {
			if (nextOptions.disabled && !options.disabled && (frame || endTimer)) {
				if (frame) {
					cancelAnimationFrame(frame);
					flush();
				}
				options.onEnd?.();
			}
			options = nextOptions;
			if (options.disabled) cancelPending();
		},
		destroy() {
			cancelPending();
			node.removeEventListener('wheel', handleWheel);
		}
	};
}
