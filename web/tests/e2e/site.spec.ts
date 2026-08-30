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
	await expect(page.getByText(/no upload/i)).toBeVisible();
	await expect(page.locator('input[type=file]')).toHaveAttribute('accept', /\.sdocx/);
	await expect(page.getByRole('link', { name: 'convert' })).toHaveAttribute('aria-current', 'page');
	expect(remoteRequests).toEqual([]);
});

test('regression suite exposes explicit, user-triggered runs', async ({ page }) => {
	await page.goto('/regressions');

	await expect(page).toHaveTitle(/regression/i);
	await expect(page.getByRole('heading', { name: /Regression lab/i })).toBeVisible();
	await expect(page.getByRole('button', { name: /Run selected/i })).toBeVisible();
	await expect(page.getByRole('link', { name: 'regressions' })).toHaveAttribute(
		'aria-current',
		'page'
	);
});

test('theme preference survives navigation', async ({ page }) => {
	await page.addInitScript(() => localStorage.setItem('sdocx-theme', 'light'));
	await page.goto('/');
	await page.getByRole('button', { name: 'Use dark theme' }).click();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');

	await page.getByRole('link', { name: 'regressions' }).click();
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
	await expect(page.getByText('No parser warnings')).toBeVisible();

	const downloadStarted = page.waitForEvent('download');
	await page.getByRole('button', { name: /^SVG/ }).click();
	const download = await downloadStarted;
	expect(download.suggestedFilename()).toBe('01-basic-formatting-page-001.svg');
	expect(remoteRequests).toEqual([]);
});
