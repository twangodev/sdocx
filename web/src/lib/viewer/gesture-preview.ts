import { rasterSize } from './raster-size';
import { CanvasCache } from './canvas-cache';
interface GesturePreviewOptions {
	src: string;
	active: boolean;
	suppressed?: boolean;
}

// Only nearby pages are cached, with at most 16 MiB per backing store.
const caches = new CanvasCache<
	HTMLCanvasElement,
	{ canvas: HTMLCanvasElement }
>(48 * 1024 * 1024, 3);

export function gesturePreview(
	image: HTMLImageElement,
	initial: GesturePreviewOptions
) {
	let options = initial;
	let nearby = false;
	let ready = false;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let idle: number | undefined;
	const canvas = document.createElement('canvas');
	canvas.setAttribute('aria-hidden', 'true');
	canvas.dataset.gesturePreview = '';
	canvas.width = canvas.height = 0;
	canvas.style.cssText = 'display:none;position:absolute;pointer-events:none;';
	image.after(canvas);
	const visibility = image.style.visibility;

	function cancel() {
		clearTimeout(timer);
		if (idle !== undefined) window.cancelIdleCallback(idle);
		idle = undefined;
	}

	function hide() {
		canvas.style.display = 'none';
		image.style.visibility = visibility;
	}

	function release() {
		cancel();
		hide();
		ready = false;
		caches.delete(canvas);
		canvas.width = canvas.height = 0;
	}

	function show() {
		if (!options.active || options.suppressed || !ready) return;
		// Keep the original image's layout and zoom anchor; replace only its paint.
		canvas.style.left = `${image.offsetLeft}px`;
		canvas.style.top = `${image.offsetTop}px`;
		canvas.style.width = `${image.offsetWidth}px`;
		canvas.style.height = `${image.offsetHeight}px`;
		const style = getComputedStyle(image);
		canvas.style.boxSizing = 'border-box';
		canvas.style.border = style.border;
		canvas.style.boxShadow = style.boxShadow;
		canvas.style.background = style.background;
		canvas.style.display = 'block';
		image.style.visibility = 'hidden';
	}

	function prepare() {
		if (
			!nearby ||
			options.active ||
			ready ||
			!image.complete ||
			!image.naturalWidth
		)
			return;
		const width = image.naturalWidth;
		const height = image.naturalHeight;
		const size = rasterSize(width, height);
		canvas.width = size.width;
		canvas.height = size.height;
		try {
			const context = canvas.getContext('2d');
			if (!context) return release();
			context.drawImage(image, 0, 0, canvas.width, canvas.height);
			ready = true;
			caches.set(canvas, { canvas }, () => {
				cancel();
				hide();
				ready = false;
			});
		} catch {
			// Cache failures leave the SVG usable.
			release();
		}
	}

	function schedule() {
		cancel();
		if (!nearby || ready || options.active) return;
		timer = setTimeout(() => {
			if ('requestIdleCallback' in window) {
				idle = window.requestIdleCallback(() => {
					idle = undefined;
					prepare();
				});
			} else {
				prepare();
			}
		}, 200);
	}

	const observer = new IntersectionObserver(
		([entry]) => {
			nearby = entry.isIntersecting;
			if (options.active) return;
			if (nearby) schedule();
			else release();
		},
		{ root: image.closest('.canvas-wrap'), rootMargin: '200px' }
	);
	observer.observe(image);
	image.addEventListener('load', schedule);

	return {
		update(next: GesturePreviewOptions) {
			const sourceChanged = next.src !== options.src;
			const wasActive = options.active;
			const wasSuppressed = options.suppressed;
			options = next;
			if (sourceChanged) release();
			if (options.suppressed) {
				hide();
				cancel();
			} else if (options.active) {
				cancel();
				if (!wasActive || sourceChanged || wasSuppressed) show();
			} else {
				hide();
				if (!nearby) release();
				else schedule();
			}
		},
		destroy() {
			observer.disconnect();
			image.removeEventListener('load', schedule);
			release();
			canvas.remove();
		}
	};
}
