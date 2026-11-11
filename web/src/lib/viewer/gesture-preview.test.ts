import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { gesturePreview } from './gesture-preview';

let deliverIntersection: (entries: IntersectionObserverEntry[]) => void;
let disconnect: ReturnType<typeof vi.fn>;
let drawImage: ReturnType<typeof vi.fn>;
let canvas: HTMLCanvasElement;
let image: HTMLImageElement;

beforeEach(() => {
	vi.useFakeTimers();
	drawImage = vi.fn();
	disconnect = vi.fn();
	canvas = {
		width: 0, height: 0, dataset: {}, style: {},
		setAttribute: vi.fn(), remove: vi.fn(),
		getContext: () => ({ drawImage })
	} as unknown as HTMLCanvasElement;
	image = Object.assign(new EventTarget(), {
		style: { visibility: '' }, complete: true, naturalWidth: 100, naturalHeight: 80,
		offsetLeft: 0, offsetTop: 0, offsetWidth: 100, offsetHeight: 80,
		after: vi.fn(), closest: () => null
	}) as unknown as HTMLImageElement;
	vi.stubGlobal('document', { createElement: () => canvas });
	vi.stubGlobal('window', {});
	vi.stubGlobal('getComputedStyle', () => ({ border: '', boxShadow: '', background: '' }));
	vi.stubGlobal('IntersectionObserver', class {
		observe = vi.fn();
		disconnect = disconnect;
		constructor(callback: (entries: IntersectionObserverEntry[]) => void) {
			deliverIntersection = callback;
		}
	});
});
afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});
const intersecting = () => deliverIntersection([{ isIntersecting: true } as IntersectionObserverEntry]);

describe('gesture preview ownership', () => {
	it('does not schedule backing-store work from an observer delivery queued before destruction', () => {
		const action = gesturePreview(image, { src: 'page.svg', active: false });
		action.destroy();
		intersecting();
		vi.advanceTimersByTime(200);
		expect(drawImage).not.toHaveBeenCalled();
		expect(canvas.width).toBe(0);
		expect(canvas.height).toBe(0);
		expect(disconnect).toHaveBeenCalledOnce();
	});

	it('rejects a retained idle callback after destruction cancels its request', () => {
		let idle!: () => void;
		const cancelIdleCallback = vi.fn();
		vi.stubGlobal('window', {
			requestIdleCallback: (callback: () => void) => { idle = callback; return 1; },
			cancelIdleCallback
		});
		const action = gesturePreview(image, { src: 'page.svg', active: false });
		intersecting();
		vi.advanceTimersByTime(200);
		action.destroy();
		idle();
		expect(cancelIdleCallback).toHaveBeenCalledExactlyOnceWith(1);
		expect(drawImage).not.toHaveBeenCalled();
		expect(canvas.width).toBe(0);
	});

	it('prepares nearby pages, replaces only paint during a gesture, and releases its backing store', () => {
		const action = gesturePreview(image, { src: 'page.svg', active: false });
		intersecting();
		vi.advanceTimersByTime(200);
		expect(drawImage).toHaveBeenCalledExactlyOnceWith(image, 0, 0, 100, 80);
		action.update({ src: 'page.svg', active: true });
		expect(canvas.style.display).toBe('block');
		expect(image.style.visibility).toBe('hidden');
		action.update({ src: 'page.svg', active: true, suppressed: true });
		expect(canvas.style.display).toBe('none');
		expect(image.style.visibility).toBe('');
		action.destroy();
		expect(canvas.width).toBe(0);
		expect(canvas.height).toBe(0);
	});
});
