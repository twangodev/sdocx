import { afterEach, describe, expect, it, vi } from 'vitest';
import { DocumentZoomCamera } from './document-zoom-camera.svelte';
import { wheelZoom } from './wheel-zoom';

describe('DocumentZoomCamera', () => {
	afterEach(() => vi.unstubAllGlobals());

	it('discards a fit-page scroll after the selected page changes', () => {
		let selectedPage = 1;
		let nextFrame: FrameRequestCallback | undefined;
		vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
			nextFrame = callback;
			return 1;
		});

		const camera = new DocumentZoomCamera(() => selectedPage);
		const scrollToPage = vi.spyOn(camera, 'scrollToPage');
		camera.fitSelectedPage();
		selectedPage = 2;
		nextFrame?.(0);

		expect(scrollToPage).not.toHaveBeenCalled();
	});

	it('discards a fit-page scroll after the zoom mode changes', () => {
		let nextFrame: FrameRequestCallback | undefined;
		vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
			nextFrame = callback;
			return 1;
		});

		const camera = new DocumentZoomCamera(() => 1);
		const scrollToPage = vi.spyOn(camera, 'scrollToPage');
		camera.fitSelectedPage();
		camera.fitWidth();
		nextFrame?.(0);

		expect(scrollToPage).not.toHaveBeenCalled();
	});

	it('discards a deferred fit-page scroll after reset keeps the same selected page', () => {
		let nextFrame: FrameRequestCallback | undefined;
		vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
			nextFrame = callback;
			return 1;
		});

		const camera = new DocumentZoomCamera(() => 0);
		const scrollToPage = vi.spyOn(camera, 'scrollToPage');
		camera.fitSelectedPage();
		camera.reset();
		nextFrame?.(0);

		expect(scrollToPage).not.toHaveBeenCalled();
	});

	it('skips fit scrolling when an earlier queued wheel flush starts a gesture first', () => {
		const frames: FrameRequestCallback[] = [];
		vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
			frames.push(callback);
			return frames.length;
		});
		vi.stubGlobal('cancelAnimationFrame', vi.fn());
		const bounds = { left: 0, top: 0, width: 600, height: 800 };
		const scroller = Object.assign(new EventTarget(), {
			clientWidth: 600, clientHeight: 800, scrollWidth: 600, scrollHeight: 800,
			scrollLeft: 0, scrollTop: 0,
			getBoundingClientRect: () => bounds
		}) as unknown as HTMLDivElement;
		const camera = new DocumentZoomCamera(() => 0);
		camera.scroller = scroller;
		camera.surface = {
			getBoundingClientRect: () => bounds,
			querySelector: () => null,
			querySelectorAll: () => []
		} as unknown as HTMLDivElement;
		const scrollToPage = vi.spyOn(camera, 'scrollToPage').mockImplementation(() => {});
		const wheel = wheelZoom(scroller, { onZoom: camera.updateGesture });
		try {
			const event = Object.assign(new Event('wheel', { cancelable: true }), {
				ctrlKey: true, deltaY: -10, deltaMode: 0, clientX: 300, clientY: 400
			});
			scroller.dispatchEvent(event);
			camera.fitSelectedPage();
			expect(frames).toHaveLength(2);
			for (const frame of frames) frame(0);

			expect(camera.gestureZoom).not.toBeNull();
			expect(scrollToPage).not.toHaveBeenCalled();
		} finally {
			wheel.destroy();
		}
	});
});
