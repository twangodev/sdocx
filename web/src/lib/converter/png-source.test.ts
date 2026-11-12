import { afterEach, expect, it, vi } from 'vitest';
import { svgToPng } from './files';

afterEach(() => vi.unstubAllGlobals());

for (const backend of ['offscreen', 'document'] as const) {
	for (const failure of ['construct', 'context', 'draw', 'encode', null] as const) {
		it(`releases decoded bitmap with ${backend} canvas after ${failure ?? 'successful encoding'}`, async () => {
			const expectedError = new Error(`${failure} failed`);
			const bitmap = { width: 10, height: 20, close: vi.fn() };
			const png = new Blob(['png'], { type: 'image/png' });
			const context = { drawImage: vi.fn(() => {
				expect(bitmap.close).not.toHaveBeenCalled();
				if (failure === 'draw') throw expectedError;
			}) };
			const canvas = {
				width: 0, height: 0,
				getContext: vi.fn(() => failure === 'context' ? null : context),
				convertToBlob: vi.fn(() => failure === 'encode' ? Promise.reject(expectedError) : Promise.resolve(png)),
				toBlob: vi.fn((callback: BlobCallback) => {
					if (failure === 'encode') throw expectedError;
					callback(png);
				})
			};
			vi.stubGlobal('createImageBitmap', vi.fn().mockResolvedValue(bitmap));
			vi.stubGlobal('OffscreenCanvas', backend === 'offscreen' ? class {
				constructor(width: number, height: number) {
					if (failure === 'construct') throw expectedError;
					canvas.width = width; canvas.height = height;
					return canvas;
				}
			} : undefined);
			vi.stubGlobal('document', { createElement: vi.fn(() => {
				if (failure === 'construct') throw expectedError;
				return canvas;
			}) });
			const result = svgToPng('<svg/>', 2);
			if (failure) await expect(result).rejects.toThrow(failure === 'context'
				? 'Canvas rendering is unavailable in this browser.' : expectedError);
			else {
				await expect(result).resolves.toBe(png);
				expect([canvas.width, canvas.height]).toEqual([20, 40]);
				expect(context.drawImage).toHaveBeenCalledWith(bitmap, 0, 0, 20, 40);
			}
			expect(bitmap.close).toHaveBeenCalledOnce();
		});
	}
}

it('retains the HTML image fallback and revokes its temporary URL', async () => {
	vi.stubGlobal('createImageBitmap', vi.fn().mockRejectedValue(new Error('SVG bitmap unsupported')));
	vi.stubGlobal('OffscreenCanvas', undefined);
	let image!: FallbackImage;
	class FallbackImage {
		width = 12;
		height = 24;
		onload?: () => void;
		constructor() { image = this; }
		set src(_url: string) { queueMicrotask(() => this.onload?.()); }
	}
	vi.stubGlobal('Image', FallbackImage);
	const createUrl = vi.fn(() => 'blob:fallback');
	const revokeUrl = vi.fn();
	vi.stubGlobal('URL', { createObjectURL: createUrl, revokeObjectURL: revokeUrl });
	const drawImage = vi.fn();
	const png = new Blob(['fallback png'], { type: 'image/png' });
	vi.stubGlobal('document', { createElement: () => ({
		getContext: () => ({ drawImage }),
		toBlob: (callback: BlobCallback) => callback(png)
	}) });
	await expect(svgToPng('<svg/>', 1)).resolves.toBe(png);
	expect(drawImage).toHaveBeenCalledWith(image, 0, 0, 12, 24);
	expect(revokeUrl).toHaveBeenCalledExactlyOnceWith('blob:fallback');
});
