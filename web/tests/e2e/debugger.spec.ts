import { test, expect } from '../fixtures/browser';
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
test('SVG fountain masks preserve maximum coverage at overlaps', async ({ page, browserName }, testInfo) => {
	const stamp = (x: number) => `<g style="mix-blend-mode:lighten"><circle r="1" fill="url(#gradient)" transform="matrix(20,0,0,20,${x},32)"/></g>`;
	const bounds = 'M0,0h64v64h-64Z';
	const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">
		<defs>
			<linearGradient id="gradient" gradientUnits="userSpaceOnUse" x1="-1" x2="1">
				<stop offset="0" stop-color="rgb(7%,7%,7%)"/><stop offset=".25" stop-color="white"/>
				<stop offset=".75" stop-color="white"/><stop offset="1" stop-color="rgb(7%,7%,7%)"/>
			</linearGradient>
			<mask id="mask" maskUnits="userSpaceOnUse" x="0" y="0" width="64" height="64">
				<g style="isolation:isolate"><path fill="black" d="${bounds}"/>${stamp(32)}${stamp(40)}</g>
			</mask>
		</defs>
		<path fill="black" mask="url(#mask)" d="${bounds}"/>
	</svg>`;
	const alpha = await page.evaluate(async (svg) => {
		const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
		try {
			const image = new Image();
			image.src = url;
			await image.decode();
			const canvas = document.createElement('canvas');
			canvas.width = canvas.height = 64;
			const context = canvas.getContext('2d')!;
			context.drawImage(image, 0, 0);
			return context.getImageData(24, 32, 1, 1).data[3];
		} finally {
			URL.revokeObjectURL(url);
		}
	}, svg);
	await testInfo.attach('fountain-overlap.svg', { body: svg, contentType: 'image/svg+xml' });
	test.fail(browserName === 'firefox', 'Lighten in SVG masks overwrites an opaque stamp with the next shaded stamp');
	expect(alpha, 'The first stamp is fully opaque at this interior overlap pixel').toBe(255);
});
test('V14 fountain SVG retains visible ink when loaded as an image', async ({
	page,
	browserName
}, testInfo) => {
	test.skip(!existsSync(handwriting), 'Local handwriting fixture is absent');
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles(handwriting);
	await page.getByRole('button', { name: 'Debugger', exact: true }).click();
	await expect(page.getByRole('button', { name: 'Play', exact: true })).toBeEnabled({ timeout: 60000 });
	await page.getByRole('button', { name: 'Restart replay' }).click();
	const vector = page.locator('[data-replay-overlay] .vector-page > svg');
	await expect(vector).toHaveAttribute('data-render-version', /.+/);
	const result = await vector.evaluate(async (element) => {
		const parsed = new DOMParser().parseFromString(element.outerHTML, 'image/svg+xml');
		const mask = parsed.querySelector('mask');
		if (!mask) throw new Error('Fixture must contain shaded V14 fountain ink');
		const fill = parsed.querySelector(`[mask="url(#${mask.id})"]`);
		if (!fill) throw new Error('Fountain mask has no painted stroke');
		const svg = parsed.documentElement.cloneNode(false) as SVGSVGElement;
		svg.setAttribute('width', '128');
		svg.setAttribute('height', '128');
		const right = Number(mask.getAttribute('x')) + Number(mask.getAttribute('width'));
		const bottom = Number(mask.getAttribute('y')) + Number(mask.getAttribute('height'));
		svg.setAttribute('viewBox', `0 0 ${right + 1} ${bottom + 1}`);
		svg.append(mask.closest('defs')!.cloneNode(true), fill.cloneNode(true));
		svg.removeAttribute('style');
		for (const node of svg.querySelectorAll<SVGElement>('[style]')) {
			node.style.removeProperty('display');
		}
		const isolated = new XMLSerializer().serializeToString(svg);
		const url = URL.createObjectURL(new Blob([isolated], { type: 'image/svg+xml' }));
		try {
			const image = new Image();
			image.src = url;
			await image.decode();
			const canvas = document.createElement('canvas');
			canvas.width = canvas.height = 128;
			const context = canvas.getContext('2d')!;
			context.drawImage(image, 0, 0);
			const pixels = context.getImageData(0, 0, 128, 128).data;
			let coverage = 0;
			for (let i = 3; i < pixels.length; i += 4) coverage += pixels[i];
			return { coverage, isolated };
		} finally {
			URL.revokeObjectURL(url);
		}
	});
	await testInfo.attach('isolated-fountain.svg', {
		body: result.isolated,
		contentType: 'image/svg+xml'
	});
	test.fail(browserName === 'firefox', 'Lighten groups in the luminance mask produce blank SVG images');
	expect(result.coverage, 'An isolated fountain stroke must paint nontransparent pixels').toBeGreaterThan(0);
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
	const vector = page.locator('[data-replay-overlay] .vector-page > svg');
	await expect(vector).toHaveAttribute('data-render-version', /.+/);
	const metrics = {
		medianFrameMs: frames[Math.floor(frames.length * 0.5)],
		p95FrameMs: frames[Math.floor(frames.length * 0.95)],
		vectorNodes: await vector.locator('*').count(),
		canvasCount: await page.locator('[data-replay-overlay] canvas').count()
	};
	expect(metrics.canvasCount).toBe(0);
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
	await expect(page.locator('article.note img')).toHaveCount(1);
	await expect.poll(() => page.evaluate(() => {
		const live = (window as unknown as { debuggerLiveUrls: Set<string> }).debuggerLiveUrls;
		const thumbnails = new Set(Array.from(document.querySelectorAll<HTMLImageElement>('article.note img'), (image) => image.src));
		return [...live].filter((url) => !thumbnails.has(url));
	})).toEqual([]);
	await page.getByRole('searchbox', { name: 'Search notes' }).fill('no matching note');
	await expect.poll(() => page.evaluate(() => (window as unknown as { debuggerLiveUrls: Set<string> }).debuggerLiveUrls.size)).toBe(0);
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
	await expect(page.locator('[data-replay-overlay] .vector-page > svg')).toBeVisible();
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

test('dense vector scrubbing preserves state across reverse seeks', async ({ page }, testInfo) => {
	test.skip(!existsSync(handwriting), 'Local dense fixture required');
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles(handwriting);
	await page.getByRole('button', { name: 'Debugger', exact: true }).click();
	await expect(page.getByRole('button', { name: 'Play', exact: true })).toBeEnabled({ timeout: 60000 });
	await page.getByRole('button', { name: 'Restart replay' }).click();
	const vector = page.locator('[data-replay-overlay] .vector-page > svg');
	await expect(vector).toHaveAttribute('data-render-version', /.+/);
	const metrics = await page.evaluate(async () => {
		const slider = document.querySelector<HTMLInputElement>('[aria-label="Replay position"]')!;
		const root = document.querySelector<SVGSVGElement>('[data-replay-overlay] .vector-page > svg')!;
		const seek = async (fraction: number) => {
			const start = performance.now();
			slider.value = String(Math.floor(Number(slider.max) * fraction));
			slider.dispatchEvent(new Event('input', { bubbles: true }));
			for (let i = 0; i < 120; i++) {
				await new Promise(requestAnimationFrame);
				if (root.dataset.renderVersion === slider.value) {
					await new Promise(requestAnimationFrame);
					return performance.now() - start;
				}
			}
			throw new Error(JSON.stringify({ wanted: slider.value, previous: root.dataset.renderVersion, connected: root.isConnected, current: document.querySelector<SVGSVGElement>('[data-replay-overlay] .vector-page > svg')?.dataset.renderVersion }));
		};
		await seek(0.5);
		const state = () => {
			const copy = root.cloneNode(true) as SVGSVGElement;
			for (const node of copy.querySelectorAll<SVGElement>('[style]')) {
				if (node.style.cssText) node.setAttribute('style', node.style.cssText);
				else node.removeAttribute('style');
			}
			return copy.outerHTML;
		};
		const expected = state();
		const times = [];
		for (const fraction of [0.95, 0.2, 0.8, 0.1, 0.9, 0.4, 0.7, 0.3, 1]) {
			times.push(await seek(fraction));
		}
		const copy = root.cloneNode(true) as SVGSVGElement;
		copy.removeAttribute('style');
		const url = URL.createObjectURL(new Blob([new XMLSerializer().serializeToString(copy)], { type: 'image/svg+xml' }));
		let sameCompletedPixels: boolean;
		try {
			const image = new Image(); image.src = url; await image.decode();
			const viewer = document.querySelector<HTMLImageElement>('img[data-page-zoom-target]')!;
			const canvas = document.createElement('canvas');
			canvas.width = 462; canvas.height = Math.round(462 * image.naturalHeight / image.naturalWidth);
			const ctx = canvas.getContext('2d')!;
			ctx.drawImage(viewer, 0, 0, canvas.width, canvas.height);
			const expected = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
			ctx.clearRect(0, 0, canvas.width, canvas.height);
			ctx.drawImage(image, 0, 0, canvas.width, canvas.height);
			const actual = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
			sameCompletedPixels = actual.every((value, i) => value === expected[i]);
		} finally { URL.revokeObjectURL(url); }
		await seek(0.5);
		return {
			sameCompletedPixels,
			sameVectors: state() === expected,
			sameRoot: root === document.querySelector('[data-replay-overlay] .vector-page > svg'),
			medianSeekMs: times.sort((a, b) => a - b)[5],
			maxSeekMs: Math.max(...times),
			gradients: root.querySelectorAll('linearGradient').length,
			images: root.querySelectorAll('image').length,
			filters: root.querySelectorAll('filter').length
		};
	});
	expect(metrics.sameCompletedPixels).toBe(true);
	expect(metrics.sameVectors).toBe(true);
	expect(metrics.sameRoot).toBe(true);
	expect(metrics.gradients).toBeGreaterThan(0);
	expect(metrics.images).toBe(0);
	expect(metrics.filters).toBe(0);
	await expect(page.locator('[data-replay-overlay] canvas')).toHaveCount(0);
	console.log('Vector scrubbing', JSON.stringify(metrics));
	await testInfo.attach('vector-scrubbing-metrics', { body: JSON.stringify(metrics), contentType: 'application/json' });
	await page.getByRole('button', { name: 'Close debugger', exact: true }).click();
	await expect(vector).toHaveCount(0);
});

test.describe('vector replay at high screen density', () => {
	test.use({ deviceScaleFactor: 2 });
	test('retains vector geometry through zoom and scrolling', async ({ page }, testInfo) => {
		test.skip(!existsSync(handwriting), 'Local dense fixture required');
		const errors: string[] = [];
		page.on('pageerror', error => errors.push(error.message));
		await page.goto('/');
		await page.locator('input[type=file]').setInputFiles(handwriting);
		await page.getByRole('button', { name: 'Debugger', exact: true }).click();
		await expect(page.getByRole('button', { name: 'Play', exact: true })).toBeEnabled({ timeout: 60000 });
		await page.getByLabel('Replay position').evaluate((input: HTMLInputElement) => {
			input.value = String(Math.floor(Number(input.max) * 0.9));
			input.dispatchEvent(new Event('input', { bubbles: true }));
		});
		const vector = page.locator('[data-replay-overlay] .vector-page > svg');
		await expect(vector).toHaveAttribute('data-render-version', /.+/);
		const geometry = await vector.innerHTML();
		const initial = (await vector.boundingBox())!;
		await page.getByRole('button', { name: 'Zoom and page fit', exact: true }).click();
		await page.getByRole('menuitem', { name: '400%', exact: true }).click();
		await expect.poll(async () => (await vector.boundingBox())!.width).toBeGreaterThan(initial.width * 4);
		await page.locator('.canvas-wrap').evaluate(element => { element.scrollTop = 0; element.scrollLeft = 0; });
		const before = (await vector.boundingBox())!.y;
		await page.locator('.canvas-wrap').evaluate(element => { element.scrollTop += 450; });
		await expect.poll(async () => (await vector.boundingBox())!.y).toBeLessThan(before - 400);
		expect(await vector.innerHTML()).toBe(geometry);
		await expect(page.locator('[data-replay-overlay] canvas')).toHaveCount(0);
		await page.screenshot({ path: testInfo.outputPath('zoomed-vector-replay.png') });
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
	const background = page.locator('[data-replay-overlay] .vector-page > svg');
	await expect(background).toHaveAttribute('data-render-version', /.+/);
	const replaySvg = await background.evaluate(element => new XMLSerializer().serializeToString(element));
	const templatePath = (value: string) =>
		value.match(/<path data-page-template="dots"[^>]*\/>/)?.[0];
	expect(templatePath(replaySvg)).toBe(templatePath(svg));
	expect(replaySvg.match(/<path d=/g)).toHaveLength(6);
	const pixels = await background.evaluate(async element => {
		const copy = element.cloneNode(true) as SVGSVGElement;
		copy.removeAttribute('style');
		const svg = new XMLSerializer().serializeToString(copy);
		const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
		try {
			const image = new Image(); image.src = url; await image.decode();
			const canvas = document.createElement('canvas');
			canvas.width = image.naturalWidth; canvas.height = image.naturalHeight;
			const context = canvas.getContext('2d')!;
			context.drawImage(image, 0, 0);
			return {
				dot: Array.from(context.getImageData(917, 1054, 1, 1).data),
				margin: Array.from(context.getImageData(917, 20, 1, 1).data)
			};
		} finally { URL.revokeObjectURL(url); }
	});
	expect(pixels.dot[3]).toBe(255);
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
