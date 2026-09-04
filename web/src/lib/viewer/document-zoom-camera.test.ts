import { afterEach, describe, expect, it, vi } from 'vitest';
import { DocumentZoomCamera } from './document-zoom-camera.svelte';

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
});
