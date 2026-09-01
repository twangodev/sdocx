import { expect, test } from '@playwright/test';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

const localFixture = process.env.SDOCX_E2E_FIXTURE ?? resolve('../hf/01-basic-formatting.sdocx');

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
	await expect(overlay).toContainText('drop .sdocx to open');
	const bounds = await overlay.boundingBox();
	expect(bounds).not.toBeNull();
	expect(bounds?.x).toBe(0);
	expect(bounds?.y).toBe(0);
	expect(bounds?.width).toBe(page.viewportSize()?.width);
	expect(bounds?.height).toBe(page.viewportSize()?.height);

	await page.locator('body').dispatchEvent('dragleave', { dataTransfer });
	await expect(overlay).toBeHidden();
});

test('regression suite exposes explicit, user-triggered runs', async ({ page }) => {
	await page.goto('/regressions');

	await expect(page).toHaveTitle(/regression/i);
	await expect(page.getByRole('heading', { name: /Regression lab/i })).toBeVisible();
	await expect(page.getByRole('button', { name: /Run selected/i })).toBeVisible();
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
	await page.getByRole('button', { name: 'Document information' }).click();
	await expect(page.getByText('No parser warnings')).toBeVisible();
	await page.getByRole('button', { name: 'Document information' }).click();
	await expect(page.getByRole('complementary', { name: 'Document information' })).toHaveCount(0);
	await expect(pageStack).toHaveAttribute('data-zoom', 'page');
	const colorModes = [
		page.getByRole('radio', { name: 'Automatic color mode' }),
		page.getByRole('radio', { name: 'Light document mode' }),
		page.getByRole('radio', { name: 'Dark document mode' })
	];
	for (const mode of colorModes) await expect(mode.locator('svg')).toHaveCount(1);
	await expect(colorModes[0]).toHaveAttribute('data-state', 'on');
	await page.getByRole('button', { name: 'Zoom in' }).click();
	await expect(pageStack).toHaveAttribute('data-zoom', '125');
	const zoomMenu = page.getByRole('button', { name: 'Zoom and page fit' });
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
