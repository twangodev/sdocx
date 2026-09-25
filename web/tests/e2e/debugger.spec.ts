import { test, expect } from '@playwright/test';
import { resolve } from 'node:path';
import { existsSync } from 'node:fs';
const fixture = resolve('../hf/01-basic-formatting.sdocx');
const handwriting =
	process.env.SDOCX_DEBUG_FIXTURE ??
	resolve('../tmp/stroke-conformance/handwritten.sdocx');
test.beforeEach(async ({ page }) => {
	await page.route('https://rybbit.twango.dev/api/script.js', (r) =>
		r.fulfill({ body: '' })
	);
});
test('debugger exposes physical pages, metadata and original source bytes', async ({
	page
}) => {
	test.skip(!existsSync(fixture), 'Local fixture is absent');
	const errors: string[] = [];
	page.on('pageerror', (e) => errors.push(e.message));
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles(fixture);
	await page.getByRole('button', { name: 'Debugger', exact: true }).click();
	await expect(
		page.getByRole('heading', { name: 'Stored pages' })
	).toBeVisible();
	await page.getByRole('button', { name: /Page 1 ·/ }).click();
	await expect(
		page.getByRole('button', { name: /Layer 0/ }).first()
	).toBeVisible();
	await page.getByRole('button', { name: 'Original file bytes' }).click();
	await expect(page.locator('.hex')).toContainText('50 4b 03 04');
	await page.getByLabel('Byte offset').fill('16');
	await page.getByLabel('Byte offset').press('Tab');
	await expect(page.locator('.hex')).toContainText('00000010');
	await page.getByRole('button', { name: 'Tree', exact: true }).click();
	await page
		.getByRole('button', { name: 'Document metadata & diagnostics' })
		.click();
	await page.getByRole('button', { name: /Properties \(/ }).click();
	await expect(
		page.getByRole('button', { name: /diagnostics \(/ })
	).toBeVisible();
	await page
		.getByRole('button', { name: 'Close debugger', exact: true })
		.click();
	await expect(
		page.getByRole('complementary', { name: 'File debugger', exact: true })
	).toHaveCount(0);
	await page.locator('input[type=file]').setInputFiles(fixture);
	await page.getByRole('button', { name: 'Debugger', exact: true }).click();
	await expect(page.getByLabel('Debugger page')).toBeVisible();
	expect(errors).toEqual([]);
});
test('handwriting replay scrubs and selects source samples', async ({
	page
}) => {
	test.skip(!existsSync(handwriting), 'Local handwriting fixture is absent');
	const errors: string[] = [];
	page.on('pageerror', (e) => errors.push(e.message));
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles(handwriting);
	await page.getByRole('button', { name: 'Debugger', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'Play', exact: true })
	).toBeEnabled({ timeout: 60000 });
	await page.getByRole('button', { name: 'Restart replay' }).click();
	await page.getByRole('button', { name: 'Next stroke' }).click();
	await expect(
		page.getByRole('heading', { name: /Stroke samples/ })
	).toBeVisible();
	await expect(page.locator('.hex')).toContainText('0000');
	await page.getByTitle('Sample 0', { exact: true }).click();
	await page.getByRole('button', { name: 'Play', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'Pause', exact: true })
	).toBeVisible();
	await page.getByRole('button', { name: 'Pause', exact: true }).click();
	await page.getByLabel('Playback speed').selectOption('2');
	await page
		.getByRole('button', { name: 'Close debugger', exact: true })
		.click();
	expect(errors).toEqual([]);
});
test('debugger switches panels at narrow widths', async ({ page }) => {
	test.skip(!existsSync(fixture), 'Local fixture is absent');
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles(fixture);
	await page.getByRole('button', { name: 'Debugger', exact: true }).click();
	await page.getByRole('button', { name: 'Tree', exact: true }).click();
	await expect(page.getByLabel('Search file tree')).toBeVisible();
	await page.getByRole('button', { name: 'Original file bytes' }).click();
	await expect(page.locator('.hex')).toContainText('50 4b 03 04');
});

test('dense replay measures frame cadence and releases browser resources', async ({
	page,
	browserName
}, testInfo) => {
	test.skip(
		browserName !== 'chromium' || !existsSync(handwriting),
		'Chromium with local dense fixture required'
	);
	await page.goto('/');
	await page.evaluate(() => {
		const live = new Set<string>();
		const create = URL.createObjectURL.bind(URL),
			revoke = URL.revokeObjectURL.bind(URL);
		URL.createObjectURL = (object) => {
			const url = create(object);
			live.add(url);
			return url;
		};
		URL.revokeObjectURL = (url) => {
			live.delete(url);
			revoke(url);
		};
		Object.assign(window, { debuggerLiveUrls: live });
	});
	await page.locator('input[type=file]').setInputFiles(handwriting);
	await page.getByRole('button', { name: 'Debugger', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'Play', exact: true })
	).toBeEnabled({ timeout: 60000 });
	await page
		.getByLabel('Replay position')
		.evaluate((element: HTMLInputElement) => {
			element.value = String(Number(element.max) * 0.75);
			element.dispatchEvent(new Event('input', { bubbles: true }));
		});
	// Let the completed-stroke cache settle before measuring continuous playback.
	await page.evaluate(
		() =>
			new Promise((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(resolve))
			)
	);
	await page.getByRole('button', { name: 'Play', exact: true }).click();
	const frames = await page.evaluate(async () => {
		const values: number[] = [];
		let last = performance.now();
		for (let i = 0; i < 120; i++) {
			await new Promise(requestAnimationFrame);
			const now = performance.now();
			values.push(now - last);
			last = now;
		}
		return values.slice(5).sort((a, b) => a - b);
	});
	await page.getByRole('button', { name: 'Pause', exact: true }).click();
	const surface = await page
		.locator('[data-replay-overlay] canvas')
		.evaluate((canvas: HTMLCanvasElement) => ({
			width: canvas.width,
			height: canvas.height
		}));
	const metrics = {
		medianFrameMs: frames[Math.floor(frames.length * 0.5)],
		p95FrameMs: frames[Math.floor(frames.length * 0.95)],
		canvasBytes: surface.width * surface.height * 4 * 2
	};
	console.log('Debugger dense replay', JSON.stringify(metrics));
	await testInfo.attach('replay-metrics', {
		body: JSON.stringify(metrics, null, 2),
		contentType: 'application/json'
	});
	await page.screenshot({ path: testInfo.outputPath('debugger.png') });
	await page
		.getByRole('button', { name: 'Back to library', exact: true })
		.click();
	await expect(page.locator('[data-replay-overlay] canvas')).toHaveCount(0);
	await expect
		.poll(() =>
			page.evaluate(
				() =>
					(window as unknown as { debuggerLiveUrls: Set<string> })
						.debuggerLiveUrls.size
			)
		)
		.toBe(0);
});

test('sidebar preserves the viewer images, camera and gesture cache', async ({
	page
}) => {
	test.skip(!existsSync(handwriting), 'Local handwriting fixture is absent');
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles(handwriting);
	const image = page.locator(
		'[data-page-index="0"] img[data-page-zoom-target]'
	);
	await expect(image).toBeVisible({ timeout: 60000 });
	await expect
		.poll(() =>
			page
				.locator('[data-page-index="0"] [data-gesture-preview]')
				.evaluate((canvas: HTMLCanvasElement) => canvas.width)
		)
		.toBeGreaterThan(0);
	await page
		.getByRole('button', { name: 'Zoom and page fit', exact: true })
		.click();
	await page.getByRole('menuitem', { name: 'Fit width', exact: true }).click();
	const zoom = await page.locator('.page-stack').getAttribute('data-zoom');
	const before = await image.evaluate((element) => {
		Object.assign(window, {
			retainedViewerImage: element,
			retainedGestureCanvas: element.parentElement?.querySelector(
				'[data-gesture-preview]'
			)
		});
		return element.getAttribute('src');
	});
	await page.getByRole('button', { name: 'Debugger', exact: true }).click();
	await expect(
		page.getByRole('complementary', { name: 'File debugger', exact: true })
	).toBeVisible();
	await expect(
		page.getByRole('button', { name: 'Zoom and page fit', exact: true })
	).toBeVisible();
	await expect(page.locator('[data-replay-overlay] canvas')).toHaveCount(0);
	await expect(image).toHaveAttribute('src', before!);
	await expect(page.locator('.page-stack')).toHaveAttribute('data-zoom', zoom!);
	expect(
		await image.evaluate(
			(element) =>
				element ===
				(window as unknown as { retainedViewerImage: Element })
					.retainedViewerImage
		)
	).toBe(true);
	expect(
		await page
			.locator('[data-page-index="0"] [data-gesture-preview]')
			.evaluate(
				(element) =>
					element ===
					(window as unknown as { retainedGestureCanvas: Element })
						.retainedGestureCanvas
			)
	).toBe(true);
	await expect(
		page.getByRole('button', { name: 'Play', exact: true })
	).toBeEnabled();
	await page
		.getByRole('button', { name: 'Restart replay', exact: true })
		.click();
	await expect(page.locator('[data-replay-overlay] img')).toBeVisible();
	await expect(image).toHaveCSS('visibility', 'hidden');
	await page.locator('.canvas-wrap').evaluate((element) => {
		element.dispatchEvent(
			new WheelEvent('wheel', {
				deltaY: -10,
				ctrlKey: true,
				cancelable: true,
				bubbles: true,
				clientX: 500,
				clientY: 300
			})
		);
	});
	await expect(
		page.locator('[data-page-index="0"] [data-gesture-preview]')
	).toHaveCSS('display', 'none');
	await expect(image).toHaveAttribute('src', before!);
	await page
		.getByRole('button', { name: 'Close debugger', exact: true })
		.click();
	await expect(page.locator('[data-replay-overlay]')).toHaveCount(0);
	await expect(image).toHaveCSS('visibility', 'visible');
	expect(
		await image.evaluate(
			(element) =>
				element ===
				(window as unknown as { retainedViewerImage: Element })
					.retainedViewerImage
		)
	).toBe(true);
	await expect(
		page.locator('[data-page-index="0"] [data-gesture-preview]')
	).toHaveCount(1);
});

test('sidebar and existing page navigation stay synchronized', async ({
	page
}) => {
	test.skip(!existsSync(fixture), 'Local fixture is absent');
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles(fixture);
	await page.getByRole('button', { name: 'Debugger', exact: true }).click();
	await expect(page.getByLabel('Page number', { exact: true })).toBeEnabled();
	await page.getByLabel('Page number', { exact: true }).fill('3');
	await page.getByLabel('Page number', { exact: true }).press('Enter');
	await page.getByLabel('Page number', { exact: true }).press('Tab');
	await expect(page.getByLabel('Debugger page')).toHaveValue('2');
	await page.getByLabel('Debugger page').selectOption('1');
	await expect(page.getByLabel('Page number', { exact: true })).toHaveValue(
		'2'
	);
	await page.getByLabel('Debugger page').selectOption('5');
	await expect(
		page.getByText('This stored page has no visible preview.')
	).toBeVisible();
	await expect(page.getByLabel('Page number', { exact: true })).toHaveValue(
		'2'
	);
});

test('dense scrubbing measures reverse seeks and preserves pixels', async ({
	page,
	browserName
}, testInfo) => {
	test.skip(
		browserName !== 'chromium' || !existsSync(handwriting),
		'Chromium with dense fixture required'
	);
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles(handwriting);
	await page.getByRole('button', { name: 'Debugger', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'Play', exact: true })
	).toBeEnabled({ timeout: 60000 });
	await page.evaluate(() => {
		const surfaces: HTMLCanvasElement[] = [];
		const original = document.createElement.bind(document);
		document.createElement = ((
			tag: string,
			options?: ElementCreationOptions
		) => {
			const element = original(tag, options);
			if (tag === 'canvas') surfaces.push(element as HTMLCanvasElement);
			return element;
		}) as typeof document.createElement;
		Object.assign(window, { replaySurfaces: surfaces });
	});
	await page.getByRole('button', { name: 'Restart replay' }).click();
	await expect(page.locator('[data-replay-overlay] canvas')).toBeVisible();
	const metrics = await page.evaluate(async () => {
		const slider = document.querySelector<HTMLInputElement>(
			'[aria-label="Replay position"]'
		)!;
		let strokes = 0;
		let fills = 0;
		const originalFill = CanvasRenderingContext2D.prototype.fill;
		CanvasRenderingContext2D.prototype.fill = function (...args: unknown[]) {
			fills++;
			return Reflect.apply(originalFill, this, args);
		};
		const original = CanvasRenderingContext2D.prototype.stroke;
		CanvasRenderingContext2D.prototype.stroke = function (path?: Path2D) {
			strokes++;
			return Reflect.apply(original, this, path ? [path] : []);
		};
		const seek = async (fraction: number) => {
			const start = performance.now();
			slider.value = String(Math.floor(Number(slider.max) * fraction));
			slider.dispatchEvent(new Event('input', { bubbles: true }));
			for (let i = 0; i < 600; i++) {
				await new Promise(requestAnimationFrame);
				const canvas = document.querySelector<HTMLCanvasElement>(
					'[data-replay-overlay] canvas'
				);
				if (canvas?.dataset.renderVersion?.split(':')[0] === slider.value) {
					await new Promise(requestAnimationFrame);
					return performance.now() - start;
				}
			}
			throw new Error('Seek did not render its latest position');
		};
		const hash = () => {
			const canvas = document.querySelector<HTMLCanvasElement>(
				'[data-replay-overlay] canvas'
			)!;
			const bytes = canvas
				.getContext('2d')!
				.getImageData(0, 0, canvas.width, canvas.height).data;
			let value = 2166136261;
			for (const byte of bytes) value = Math.imul(value ^ byte, 16777619);
			return value >>> 0;
		};
		try {
			const background = document.querySelector<HTMLImageElement>(
				'[data-replay-overlay] img'
			)!.src;
			const coldSeekMs = await seek(0.95);
			await seek(0.5);

			const startStrokes = strokes;
			const startFills = fills;
			const values = [];
			for (let i = 0; i < 24; i++) values.push(await seek(0.9 - i * 0.015));
			const warmStrokes = strokes - startStrokes;
			const warmFills = fills - startFills;
			// Rapidly changing targets must settle on the latest request.
			for (let i = 0; i < 12; i++) {
				slider.value = String(
					Math.floor(Number(slider.max) * (i % 2 ? 0.8 : 0.1))
				);
				slider.dispatchEvent(new Event('input', { bubbles: true }));
				await new Promise(requestAnimationFrame);
			}
			await seek(0.5);
			const expected = hash();
			// Visiting the full-page view must not throw away the replay caches.
			slider.value = slider.max;
			slider.dispatchEvent(new Event('input', { bubbles: true }));
			await new Promise(requestAnimationFrame);
			await new Promise(requestAnimationFrame);
			const beforeReturn = strokes;
			const fillsBeforeReturn = fills;
			await seek(0.2);
			await seek(0.5);
			return {
				coldSeekMs,
				medianSeekMs: values.sort((a, b) => a - b)[12],
				p95SeekMs: values[22],
				warmStrokes,
				warmFills,
				returnFills: fills - fillsBeforeReturn,
				returnStrokes: strokes - beforeReturn,
				stableBackground:
					background ===
					document.querySelector<HTMLImageElement>('[data-replay-overlay] img')!
						.src,
				cacheBytes: (
					window as unknown as { replaySurfaces: HTMLCanvasElement[] }
				).replaySurfaces.reduce(
					(bytes, canvas) => bytes + canvas.width * canvas.height * 4,
					0
				),
				samePixels: expected === hash()
			};
		} finally {
			CanvasRenderingContext2D.prototype.stroke = original;
			CanvasRenderingContext2D.prototype.fill = originalFill;
		}
	});
	console.log('Debugger dense scrubbing', JSON.stringify(metrics));
	await testInfo.attach('scrubbing-metrics', {
		body: JSON.stringify(metrics),
		contentType: 'application/json'
	});
	expect(metrics.samePixels).toBe(true);
	// The fixture previously issued nearly five million segment draws here.
	expect(metrics.warmStrokes + metrics.warmFills).toBeLessThan(150000);
	expect(metrics.returnStrokes + metrics.returnFills).toBeLessThan(15000);
	expect(metrics.stableBackground).toBe(true);
	expect(metrics.cacheBytes).toBeLessThanOrEqual(112 * 1024 * 1024);
	await page
		.getByRole('button', { name: 'Close debugger', exact: true })
		.click();
	expect(
		await page.evaluate(() =>
			(
				window as unknown as { replaySurfaces: HTMLCanvasElement[] }
			).replaySurfaces.every(
				(canvas) => canvas.width === 0 && canvas.height === 0
			)
		)
	).toBe(true);
});

test.describe('replay at high screen density', () => {
	test.use({ deviceScaleFactor: 2 });
	test('rerenders visible ink at zoom resolution and follows scrolling', async ({
		page
	}, testInfo) => {
		test.skip(!existsSync(handwriting), 'Local dense fixture required');
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		await page.goto('/');
		await page.locator('input[type=file]').setInputFiles(handwriting);
		await page.getByRole('button', { name: 'Debugger', exact: true }).click();
		await expect(
			page.getByRole('button', { name: 'Play', exact: true })
		).toBeEnabled({ timeout: 60000 });
		await page
			.getByLabel('Replay position')
			.evaluate((input: HTMLInputElement) => {
				input.value = String(Math.floor(Number(input.max) * 0.9));
				input.dispatchEvent(new Event('input', { bubbles: true }));
			});
		const canvas = page.locator('[data-replay-overlay] canvas');
		await expect(canvas).toHaveAttribute('data-raster-scale', /.+/);
		const initialScale = Number(await canvas.getAttribute('data-raster-scale'));
		await page
			.getByRole('button', { name: 'Zoom and page fit', exact: true })
			.click();
		await page.getByRole('menuitem', { name: '400%', exact: true }).click();
		await expect
			.poll(async () => Number(await canvas.getAttribute('data-raster-scale')))
			.toBeGreaterThan(initialScale * 4);
		await page.locator('.canvas-wrap').evaluate((element) => {
			element.scrollTop = 0;
			element.scrollLeft = 0;
		});
		await expect
			.poll(async () => Number(await canvas.getAttribute('data-source-y')))
			.toBeLessThan(1);
		const density = await canvas.evaluate((element: HTMLCanvasElement) => {
			const rect = element.getBoundingClientRect();
			return {
				x: element.width / rect.width,
				y: element.height / rect.height,
				dpr: devicePixelRatio,
				pixels: element.width * element.height,
				pageHeight: element
					.closest('[data-replay-overlay]')!
					.getBoundingClientRect().height,
				visibleHeight: rect.height
			};
		});
		expect(density.x).toBeGreaterThanOrEqual(density.dpr * 0.99);
		expect(density.y).toBeGreaterThanOrEqual(density.dpr * 0.99);
		expect(density.pixels).toBeLessThanOrEqual(8 * 1024 * 1024);
		expect(density.pageHeight).toBeGreaterThan(density.visibleHeight * 4);
		const before = Number(await canvas.getAttribute('data-source-y'));
		await page.locator('.canvas-wrap').evaluate((element) => {
			element.scrollTop += 450;
		});
		await expect
			.poll(async () => Number(await canvas.getAttribute('data-source-y')))
			.toBeGreaterThan(before);
		await page.screenshot({ path: testInfo.outputPath('zoomed-replay.png') });
		console.log('Zoomed replay density', JSON.stringify(density));
		expect(
			await canvas.evaluate((element: HTMLCanvasElement) =>
				element
					.getContext('2d')!
					.getImageData(0, 0, element.width, element.height)
					.data.some((value, i) => i % 4 === 3 && value > 0)
			)
		).toBe(true);
		await page
			.getByLabel('Replay position')
			.evaluate((input: HTMLInputElement) => {
				input.value = input.max;
				input.dispatchEvent(new Event('input', { bubbles: true }));
			});
		await page.screenshot({ path: testInfo.outputPath('zoomed-original.png') });
		expect(errors).toEqual([]);
	});
});

test('dotted fixture shares native geometry between the viewer and replay', async ({
	page
}, testInfo) => {
	const shapes = resolve('../hf/02-shapes-and-dot-calibration.sdocx');
	test.skip(!existsSync(shapes), 'Local shapes fixture is absent');
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	await page.goto('/');
	await page.evaluate(() => {
		const blobs = new Map<string, Blob>();
		const create = URL.createObjectURL.bind(URL);
		URL.createObjectURL = (value) => {
			const url = create(value);
			if (value instanceof Blob) blobs.set(url, value);
			return url;
		};
		Object.assign(window, { templateTestBlobs: blobs });
	});
	await page.locator('input[type=file]').setInputFiles(shapes);
	const viewer = page.locator(
		'[data-page-index="0"] img[data-page-zoom-target]'
	);
	await expect(viewer).toBeVisible({ timeout: 60000 });
	await expect(page.locator('img[data-page-zoom-target]')).toHaveCount(1);
	await expect
		.poll(() =>
			viewer.evaluate(
				(image: HTMLImageElement) => image.complete && image.naturalWidth > 0
			)
		)
		.toBe(true);
	const svg = await viewer.evaluate((image: HTMLImageElement) =>
		(
			window as unknown as { templateTestBlobs: Map<string, Blob> }
		).templateTestBlobs
			.get(image.src)!
			.text()
	);
	expect(svg).toContain('data-page-template="dots"');
	expect(svg).toContain('M 466.05 403.90');
	expect(svg).toContain('M 678.82 398.50');
	// Each of the 77 FountainPen strokes uses one native circular-stamp path.
	expect(svg.match(/<path fill="[^"]+" d="M[^"]+a/g)).toHaveLength(77);
	await page.getByRole('button', { name: 'Debugger', exact: true }).click();
	await expect(page.getByLabel('Debugger page').locator('option')).toHaveCount(
		2
	);
	await expect(
		page.getByRole('button', { name: 'Play', exact: true })
	).toBeEnabled();
	await page.getByRole('button', { name: 'Restart replay' }).click();
	const background = page.locator('[data-replay-overlay] img');
	await expect(background).toBeVisible();
	await expect
		.poll(() =>
			background.evaluate(
				(image: HTMLImageElement) => image.complete && image.naturalWidth > 0
			)
		)
		.toBe(true);
	const replaySvg = await background.evaluate((image: HTMLImageElement) =>
		(
			window as unknown as { templateTestBlobs: Map<string, Blob> }
		).templateTestBlobs
			.get(image.src)!
			.text()
	);
	const templatePath = (value: string) =>
		value.match(/<path data-page-template="dots"[^>]*\/>/)?.[0];
	expect(templatePath(replaySvg)).toBe(templatePath(svg));
	expect(replaySvg.match(/<path d=/g)).toHaveLength(6);
	const pixels = await background.evaluate((image: HTMLImageElement) => {
		const canvas = document.createElement('canvas');
		canvas.width = image.naturalWidth;
		canvas.height = image.naturalHeight;
		const context = canvas.getContext('2d')!;
		context.drawImage(image, 0, 0);
		return {
			dot: Array.from(context.getImageData(917, 1054, 1, 1).data),
			margin: Array.from(context.getImageData(917, 20, 1, 1).data)
		};
	});
	expect(pixels.dot[0]).toBeLessThan(225);
	expect(pixels.margin).toEqual([252, 252, 252, 255]);
	await page.getByRole('button', { name: 'Next stroke' }).click();
	await expect(
		page.getByRole('heading', { name: /Stroke samples/ })
	).toBeVisible();
	await page.getByRole('button', { name: /Properties \(/ }).click();
	await page.getByRole('button', { name: /rendering \(/ }).click();
	await expect(page.getByText('reconstructed', { exact: true })).toBeVisible();
	await page.screenshot({
		path: testInfo.outputPath('dot-template-replay.png')
	});
	await page.getByLabel('Debugger page').selectOption('1');
	await expect(
		page.getByText('This stored page has no visible preview.')
	).toBeVisible();
	await expect(page.locator('img[data-page-zoom-target]')).toHaveCount(1);
	expect(errors).toEqual([]);
});
