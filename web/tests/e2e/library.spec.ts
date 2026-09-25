import { expect, test } from '@playwright/test';
import { pdfNote } from '../fixtures/pdf-note';

const note = (name = 'library-note.sdocx', threePages = false) => ({ name, mimeType: 'application/zip', buffer: pdfNote(false, threePages) });
test.beforeEach(async ({ page }) => {
	await page.route('https://rybbit.twango.dev/**', (route) => route.fulfill({ body: '' }));
	await page.goto('/');
	await expect(page.getByRole('button', { name: 'Import notes', exact: true })).toBeEnabled();
});

test('persists original notes across reloads and reopens them for export', async ({ page }) => {
	await page.locator('input[type=file]').setInputFiles(note());
	await expect(page.getByRole('region', { name: 'Document converter' })).toBeVisible();
	await page.getByRole('button', { name: 'Back to library' }).click();
	await expect(page.locator('article.note')).toHaveCount(1);
	await expect(page.locator('article.note img')).toBeVisible();
	await page.reload();
	await expect(page.locator('article.note')).toHaveCount(1);
	await page.getByRole('button', { name: /^Open / }).click();
	await expect(page.getByRole('region', { name: 'Document converter' })).toBeVisible();
	await page.getByRole('button', { name: 'Export document', exact: true }).click();
	await expect(page.getByRole('dialog', { name: 'Export document' })).toBeVisible();
	const download = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Download PDF', exact: true }).click();
	expect((await download).suggestedFilename()).toMatch(/\.pdf$/);
});

test('batch import keeps successes, identifies duplicates, and reports invalid files', async ({ page }) => {
	await page.locator('input[type=file]').setInputFiles([
		note(), note('copy.sdocx'), note('second.sdocx', true),
		{ name: 'broken.sdocx', mimeType: 'application/zip', buffer: Buffer.from('broken') }
	]);
	await expect(page.locator('.import-status')).toContainText('2 imported · 1 duplicates · 1 failed');
	await expect(page.locator('article.note')).toHaveCount(2);
	await page.getByRole('searchbox', { name: 'Search notes' }).fill('second');
	await expect(page.locator('article.note')).toHaveCount(1);
	await page.getByRole('button', { name: 'List view' }).click();
	await expect(page.locator('.notes')).toHaveClass(/list/);
});

test('concurrent tabs deduplicate imports and refresh their libraries', async ({ page, context }) => {
	const other = await context.newPage();
	await other.goto('/');
	await expect(other.getByRole('button', { name: 'Import notes', exact: true })).toBeEnabled();
	await Promise.all([
		page.locator('input[type=file]').setInputFiles([note(), note('copy.sdocx')]),
		other.locator('input[type=file]').setInputFiles([note(), note('copy.sdocx')])
	]);
	await expect(page.locator('article.note')).toHaveCount(1);
	await expect(other.locator('article.note')).toHaveCount(1);
	await expect(page.getByRole('button', { name: 'Cancel import' })).toHaveCount(0);
	await expect(other.getByRole('button', { name: 'Cancel import' })).toHaveCount(0);
	await other.close();
});

test('mobile navigation and themes remain usable', async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.getByRole('button', { name: 'Library navigation', exact: true }).click();
	await page.getByRole('navigation', { name: 'Library', exact: true }).getByRole('button', { name: /Favorites/ }).click();
	await expect(page.getByRole('heading', { name: 'Favorites', exact: true })).toBeVisible();
	await expect(page.getByRole('complementary', { name: 'Library sidebar' })).toBeHidden();
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
	await page.getByRole('button', { name: 'Use light theme' }).click();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
	await page.getByRole('button', { name: 'Use dark theme' }).click();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});
