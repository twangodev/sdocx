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

test('organizes notes without copying originals and distinguishes collection removal from deletion', async ({ page }) => {
	await page.getByRole('button', { name: 'Create collection', exact: true }).click();
	await page.getByRole('textbox', { name: 'Collection name' }).fill('Course');
	await page.getByRole('button', { name: 'Save collection' }).click();
	await page.getByRole('navigation', { name: 'Collections', exact: true }).getByRole('button', { name: /Course/ }).click();
	await page.locator('input[type=file]').setInputFiles([note(), note('second.sdocx', true)]);
	await expect(page.locator('article.note')).toHaveCount(2);
	await expect(page.getByRole('button', { name: 'Cancel import' })).toHaveCount(0);
	await page.getByRole('checkbox', { name: 'Select all visible notes' }).check();
	await page.getByRole('button', { name: 'Favorite', exact: true }).click();
	await expect(page.locator('article.note button[aria-pressed=true]')).toHaveCount(2);
	await page.getByRole('button', { name: 'Remove from collection', exact: true }).click();
	await expect(page.locator('article.note')).toHaveCount(0);
	await page.getByRole('button', { name: 'Clear selection' }).click();
	await page.getByRole('button', { name: 'Rename collection' }).click();
	await page.getByRole('textbox', { name: 'Collection name' }).fill('Renamed course');
	await page.getByRole('button', { name: 'Save collection' }).click();
	await expect(page.getByRole('heading', { name: 'Renamed course' })).toBeVisible();
	await page.getByRole('button', { name: 'Delete collection', exact: true }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'Delete collection', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'All notes', exact: true })).toBeVisible();
	await expect(page.locator('article.note')).toHaveCount(2);
	await page.locator('article.note input[type=checkbox]').first().check();
	const download = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Download original' }).click();
	expect((await download).suggestedFilename()).toMatch(/\.sdocx$/);
	await page.getByRole('checkbox', { name: 'Select all visible notes' }).check();
	await page.getByRole('button', { name: 'Delete from library', exact: true }).click();
	await page.getByRole('button', { name: 'Delete notes', exact: true }).click();
	await expect(page.locator('article.note')).toHaveCount(0);
	await page.reload();
	await expect(page.getByRole('heading', { name: 'Your notes, in one place' })).toBeVisible();
	const assets = await page.evaluate(async () => {
		const root = await (await navigator.storage.getDirectory()).getDirectoryHandle('sdocx');
		const names = [];
		for (const kind of ['originals', 'thumbnails']) {
			const directory = await root.getDirectoryHandle(kind);
			for await (const name of (directory as FileSystemDirectoryHandle & { keys(): AsyncIterable<string> }).keys()) names.push(name);
		}
		return names;
	});
	expect(assets).toEqual([]);
});
