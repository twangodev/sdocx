import { expect, test, type Locator } from '@playwright/test';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

const localFixture = process.env.SDOCX_E2E_FIXTURE ?? resolve('../hf/01-basic-formatting.sdocx');
const zoomAnchor = { x: 500, y: 400 } as const;

interface ImagePoint {
	page?: string;
	x: number;
	y: number;
}

async function pinchAtAnchor(canvas: Locator, deltaY: number) {
	return canvas.evaluate(
		async (element, gesture) => {
			const pointAtAnchor = () => {
				const pageAtPoint = document
					.elementFromPoint(gesture.x, gesture.y)
					?.closest<HTMLElement>('[data-page-index]');
				const image = pageAtPoint?.querySelector<HTMLImageElement>('img');
				if (!pageAtPoint || !image) return undefined;
				const bounds = image.getBoundingClientRect();
				return {
					page: pageAtPoint.dataset.pageIndex,
					x: (gesture.x - bounds.left) / bounds.width,
					y: (gesture.y - bounds.top) / bounds.height
				};
			};
			const pointBefore = pointAtAnchor();
			element.dispatchEvent(
				new WheelEvent('wheel', {
					deltaY: gesture.deltaY,
					ctrlKey: true,
					cancelable: true,
					clientX: gesture.x,
					clientY: gesture.y
				})
			);
			await new Promise<void>((resolveFrame) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolveFrame()))
			);
			const stack = element.querySelector<HTMLElement>('.page-stack');
			return {
				pointBefore,
				pointDuring: pointAtAnchor(),
				zoom: stack?.dataset.zoom,
				transform: stack ? getComputedStyle(stack).transform : 'none'
			};
		},
		{ ...zoomAnchor, deltaY }
	);
}

function expectSameImagePoint(actual: ImagePoint | undefined, expected: ImagePoint | undefined): void {
	expect(expected).toBeDefined();
	expect(actual?.page).toBe(expected?.page);
	expect(actual?.x).toBeCloseTo(expected?.x ?? 0, 2);
	expect(actual?.y).toBeCloseTo(expected?.y ?? 0, 2);
}

async function surfaceCenterOffset(canvas: Locator): Promise<number> {
	return canvas.evaluate((element) => {
		const surface = element.querySelector<HTMLElement>('.page-stack');
		if (!surface) return Infinity;
		const viewport = element.getBoundingClientRect();
		const bounds = surface.getBoundingClientRect();
		return Math.abs((bounds.left + bounds.right) / 2 - (viewport.left + viewport.right) / 2);
	});
}

async function expectSmoothRecenter(canvas: Locator, surface: Locator): Promise<void> {
	await expect(surface).toHaveClass(/recentering/);
	expect(await surfaceCenterOffset(canvas)).toBeGreaterThan(1);
	await expect(surface).not.toHaveClass(/recentering/);
	await expect.poll(() => surfaceCenterOffset(canvas)).toBeLessThan(1);
}

test('converter presents a local-only upload surface', async ({ page }) => {
	const remoteRequests: string[] = [];
	page.on('request', (request) => {
		const url = new URL(request.url());
		if (['http:', 'https:'].includes(url.protocol) && url.hostname !== '127.0.0.1') {
			remoteRequests.push(request.url());
		}
	});

	await page.goto('/');

	await expect(page).toHaveTitle(/local Samsung Notes converter/i);
	await expect(page.getByRole('heading', { name: /Open a Samsung note/i })).toBeVisible();
	await expect(page.locator('.lede')).toContainText('Files stay in this browser.');
	await expect(page.locator('input[type=file]')).toHaveAttribute('accept', /\.sdocx/);
	await expect(page.locator('select')).toHaveCount(0);
	await expect(page.getByRole('link', { name: 'regressions' })).toHaveCount(0);
	expect(remoteRequests).toEqual([]);
});

test('dragging a file expands the drop target across the viewport', async ({ page }) => {
	await page.goto('/', { waitUntil: 'networkidle' });
	const dataTransfer = await page.evaluateHandle(() => {
		const transfer = new DataTransfer();
		transfer.items.add(new File(['fixture'], 'dragged.sdocx', { type: 'application/zip' }));
		return transfer;
	});

	await page.locator('body').dispatchEvent('dragenter', { dataTransfer });

	const overlay = page.locator('.drop-overlay');
	await expect(overlay).toBeVisible();
	await expect.poll(() => overlay.evaluate((element) => Number(getComputedStyle(element).opacity))).toBeGreaterThan(0.9);
	await expect(overlay).toContainText('drop .sdocx to open');
	const bounds = await overlay.boundingBox();
	expect(bounds).not.toBeNull();
	expect(bounds?.x).toBe(0);
	expect(bounds?.y).toBe(0);
	expect(bounds?.width).toBe(page.viewportSize()?.width);
	expect(bounds?.height).toBe(page.viewportSize()?.height);

	await page.locator('body').dispatchEvent('dragleave', { dataTransfer });
	await expect(overlay).toBeHidden();
	expect(await overlay.evaluate((element) => Number(getComputedStyle(element).opacity))).toBeLessThan(0.1);
});

test('regression suite exposes explicit, user-triggered runs', async ({ page }) => {
	await page.goto('/regressions');

	await expect(page).toHaveTitle(/regression/i);
	await expect(page.getByRole('heading', { name: /Regression lab/i })).toBeVisible();
	await expect(page.getByRole('button', { name: /Run selected/i })).toBeVisible();
	await expect(page.getByLabel('Compatibility fixtures')).toBeVisible();
	await expect(page.getByRole('heading', { name: '01-basic-formatting' })).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Local fixture fallback' })).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Structural checks' })).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Visual comparison' })).toBeVisible();
	await expect(page.locator('select')).toHaveCount(0);
	await expect(page.getByRole('link', { name: 'regressions' })).toHaveCount(0);
});

test('theme preference survives navigation', async ({ page }) => {
	await page.goto('/');
	await page.evaluate(() => localStorage.setItem('sdocx-theme', 'light'));
	await page.reload();
	await page.getByRole('button', { name: 'Use dark theme' }).click();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');

	await page.goto('/regressions');
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('interface motion follows the reduced-motion preference', async ({ page }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.goto('/');

	const timings = await page.locator('.intro').evaluate((element) => {
		const introStyles = getComputedStyle(element);
		const overlayStyles = getComputedStyle(document.querySelector('.drop-overlay')!);
		return {
			animationSeconds: Number.parseFloat(introStyles.animationDuration),
			animationDelaySeconds: Number.parseFloat(introStyles.animationDelay),
			transitionSeconds: Number.parseFloat(overlayStyles.transitionDuration),
			transitionDelaySeconds: Number.parseFloat(overlayStyles.transitionDelay)
		};
	});

	expect(timings.animationSeconds).toBeLessThan(0.001);
	expect(timings.animationDelaySeconds).toBe(0);
	expect(timings.transitionSeconds).toBeLessThan(0.001);
	expect(timings.transitionDelaySeconds).toBe(0);
});

test('real fixture parses, renders, and exports without an upload', async ({ page }, testInfo) => {
	test.skip(testInfo.project.name !== 'chromium', 'One real WASM smoke test is sufficient.');
	test.skip(!existsSync(localFixture), 'Set SDOCX_E2E_FIXTURE or check out the external corpus.');
	const remoteRequests: string[] = [];
	page.on('request', (request) => {
		const url = new URL(request.url());
		if (['http:', 'https:'].includes(url.protocol) && url.hostname !== '127.0.0.1') {
			remoteRequests.push(request.url());
		}
	});

	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles(localFixture);

	await expect(page.getByRole('heading', { name: '01-basic-test' })).toBeVisible();
	await expect(page.getByAltText('Rendered preview of page 1')).toBeVisible();
	const pageStack = page.locator('.page-stack');
	await expect(pageStack.locator('img')).toHaveCount(5);
	await expect(page.getByRole('complementary', { name: 'Document information' })).toHaveCount(0);
	const viewerBody = page.locator('.viewer-body');
	const detailsShell = page.locator('.details-shell');
	await page.getByRole('button', { name: 'Document information' }).click();
	await expect(viewerBody).toHaveClass(/details-open/);
	await expect.poll(() => detailsShell.evaluate((element) => Number(getComputedStyle(element).opacity))).toBeGreaterThan(0.9);
	await expect(page.getByText('No parser warnings')).toBeVisible();
	await page.getByRole('button', { name: 'Document information' }).click();
	await expect(page.getByRole('complementary', { name: 'Document information' })).toHaveCount(0);
	await expect
		.poll(() => detailsShell.evaluate((element) => Number(getComputedStyle(element).opacity)))
		.toBeLessThan(0.01);
	await expect(pageStack).toHaveAttribute('data-zoom', 'page');
	const colorModes = [
		page.getByRole('radio', { name: 'Automatic color mode' }),
		page.getByRole('radio', { name: 'Light document mode' }),
		page.getByRole('radio', { name: 'Dark document mode' })
	];
	for (const mode of colorModes) await expect(mode.locator('svg')).toHaveCount(1);
	await expect(colorModes[0]).toHaveAttribute('data-state', 'on');
	const canvas = page.locator('.canvas-wrap');
	const fitPagePinch = await pinchAtAnchor(canvas, -40);
	expect(fitPagePinch.zoom).not.toBe('page');
	expect(Number(fitPagePinch.zoom)).toBeGreaterThan(0);
	expect(fitPagePinch.transform).not.toBe('none');
	expectSameImagePoint(fitPagePinch.pointDuring, fitPagePinch.pointBefore);
	await page.waitForTimeout(160);
	await expect(pageStack).toHaveAttribute('data-zoom', fitPagePinch.zoom!);
	await expect(pageStack).not.toHaveClass(/zooming/);
	await expectSmoothRecenter(canvas, pageStack);

	const manualPinch = await pinchAtAnchor(canvas, -20);
	expectSameImagePoint(manualPinch.pointDuring, manualPinch.pointBefore);
	await page.waitForTimeout(160);
	await expect(pageStack).not.toHaveClass(/zooming/);
	await expectSmoothRecenter(canvas, pageStack);

	const zoomMenu = page.getByRole('button', { name: 'Zoom and page fit' });
	await zoomMenu.click();
	await page.getByRole('menuitem', { name: 'Fit page' }).click();
	await expect(pageStack).toHaveAttribute('data-zoom', 'page');
	await page.getByRole('button', { name: 'Zoom in' }).click();
	await expect(pageStack).toHaveAttribute('data-zoom', '125');
	await zoomMenu.click();
	await page.getByRole('menuitem', { name: 'Fit page' }).click();
	await expect(pageStack).toHaveAttribute('data-zoom', 'page');
	await zoomMenu.click();
	await page.getByRole('menuitem', { name: 'Fit width' }).click();
	await expect(pageStack).toHaveAttribute('data-zoom', '100');
	await page.keyboard.press('Control+=');
	await expect(pageStack).toHaveAttribute('data-zoom', '125');
	await page.keyboard.press('Control+-');
	await expect(pageStack).toHaveAttribute('data-zoom', '100');
	await page.keyboard.press('Control+=');
	await expect(pageStack).toHaveAttribute('data-zoom', '125');
	await page.keyboard.press('Control+0');
	await expect(pageStack).toHaveAttribute('data-zoom', '100');
	await canvas.dispatchEvent('wheel', {
		deltaY: -10,
		ctrlKey: true,
		cancelable: true,
		clientX: 500,
		clientY: 400
	});
	await expect(pageStack).toHaveAttribute('data-zoom', '108.3');
	await canvas.dispatchEvent('wheel', {
		deltaY: 10,
		ctrlKey: true,
		cancelable: true,
		clientX: 500,
		clientY: 400
	});
	await expect(pageStack).toHaveAttribute('data-zoom', '100');
	await expect(pageStack).not.toHaveClass(/zooming/);
	await expect(pageStack).not.toHaveClass(/recentering/);

	const pageSelector = page.getByRole('textbox', { name: 'Page number' });
	await page.getByRole('button', { name: 'Next page' }).click();
	await expect(pageSelector).toHaveValue('2');
	await page.getByRole('button', { name: 'Previous page' }).click();
	await expect(pageSelector).toHaveValue('1');
	await page.locator('.canvas-wrap').evaluate((element) => (element.scrollTop = element.scrollHeight));
	await expect(pageSelector).toHaveValue('5');
	await pageSelector.fill('1');
	await pageSelector.press('Enter');
	await expect(pageSelector).toHaveValue('1');
	await pageSelector.blur();
	await page.keyboard.press('PageDown');
	await expect(pageSelector).toHaveValue('2');
	await page.keyboard.press('End');
	await expect(pageSelector).toHaveValue('5');
	await page.keyboard.press('Home');
	await expect(pageSelector).toHaveValue('1');

	const exportMenu = page.getByRole('button', { name: 'Export document' });
	await exportMenu.click();
	await page.getByRole('menuitem', { name: 'PNG scale: 2×' }).click();

	const downloadStarted = page.waitForEvent('download');
	await exportMenu.click();
	await page.getByRole('menuitem', { name: 'Current page as SVG' }).click();
	const download = await downloadStarted;
	expect(download.suggestedFilename()).toBe('01-basic-formatting-page-001.svg');
	expect(remoteRequests).toEqual([]);
});
