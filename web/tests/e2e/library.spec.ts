import { expect, test, type Page } from '@playwright/test';
import { pdfNote } from '../fixtures/pdf-note';

const note = (name = 'library-note.sdocx', threePages = false) => ({
	name,
	mimeType: 'application/zip',
	buffer: pdfNote(false, threePages)
});
test.beforeEach(async ({ page }) => {
	await page.route('https://rybbit.twango.dev/**', (route) => route.fulfill({ body: '' }));
	await page.goto('/');
	await expect(page.getByRole('button', { name: 'Import notes', exact: true })).toBeEnabled();
});

async function selectionAction(page: Page, name: string) {
	await page.getByRole('button', { name: 'Selection actions', exact: true }).click();
	await page.getByRole('menuitem', { name, exact: true }).click();
}
async function collectionAction(page: Page, name: string) {
	await page.getByRole('button', { name: 'Collection actions', exact: true }).click();
	await page.getByRole('menuitem', { name, exact: true }).click();
}

test('persists original notes across reloads and reopens them for export', async ({ page }) => {
	await page.locator('input[type=file]').setInputFiles(note());
	await expect(page.getByRole('region', { name: 'Document converter' })).toBeVisible();
	await expect(page.locator('[data-page-index] img').first()).toBeVisible();
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

test('batch import keeps successes, identifies duplicates, and reports invalid files', async ({
	page
}) => {
	await page
		.locator('input[type=file]')
		.setInputFiles([
			note(),
			note('copy.sdocx'),
			note('second.sdocx', true),
			{ name: 'broken.sdocx', mimeType: 'application/zip', buffer: Buffer.from('broken') }
		]);
	await expect(page.locator('.import-status')).toContainText(
		'2 imported · 1 duplicates · 1 failed'
	);
	await expect(page.locator('article.note')).toHaveCount(2);
	await page.getByRole('searchbox', { name: 'Search notes' }).fill('second');
	await expect(page.locator('article.note')).toHaveCount(1);
	await page.getByRole('radio', { name: 'List view' }).click();
	await expect(page.locator('.notes')).toHaveClass(/list/);
});

test('concurrent tabs deduplicate imports and refresh their libraries', async ({
	page,
	context
}) => {
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
	await page
		.getByRole('navigation', { name: 'Library', exact: true })
		.getByRole('button', { name: /Favorites/ })
		.click();
	await expect(page.getByRole('heading', { name: 'Favorites', exact: true })).toBeVisible();
	await expect(page.getByRole('complementary', { name: 'Library sidebar' })).toBeHidden();
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
	await page.getByRole('button', { name: 'Use light theme' }).click();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
	await page.getByRole('button', { name: 'Use dark theme' }).click();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('organizes notes without copying originals and distinguishes collection removal from deletion', async ({
	page
}) => {
	await page.getByRole('button', { name: 'Create collection', exact: true }).click();
	await page.getByRole('textbox', { name: 'Collection name' }).fill('Course');
	await page.getByRole('button', { name: 'Save collection' }).click();
	await page
		.getByRole('navigation', { name: 'Collections', exact: true })
		.getByRole('button', { name: /Course/ })
		.click();
	await page.locator('input[type=file]').setInputFiles([note(), note('second.sdocx', true)]);
	await expect(page.locator('article.note')).toHaveCount(2);
	await expect(page.getByRole('button', { name: 'Cancel import' })).toHaveCount(0);
	await page.getByRole('checkbox', { name: 'Select all visible notes' }).check();
	await expect(page.locator('article.note.selected')).toHaveCount(2);
	await selectionAction(page, 'Favorite');
	await expect(page.locator('article.note [aria-label=Favorite]')).toHaveCount(2);
	await selectionAction(page, 'Remove from collection');
	await expect(page.locator('article.note')).toHaveCount(0);
	await page.getByRole('button', { name: 'Clear selection' }).click();
	await collectionAction(page, 'Rename collection');
	await page.getByRole('textbox', { name: 'Collection name' }).fill('Renamed course');
	await page.getByRole('button', { name: 'Save collection' }).click();
	await expect(page.getByRole('heading', { name: 'Renamed course' })).toBeVisible();
	await collectionAction(page, 'Delete collection');
	await page
		.getByRole('dialog')
		.getByRole('button', { name: 'Delete collection', exact: true })
		.click();
	await expect(page.getByRole('heading', { name: 'All notes', exact: true })).toBeVisible();
	await expect(page.locator('article.note')).toHaveCount(2);
	await page.locator('article.note [role=checkbox]').first().check();
	const download = page.waitForEvent('download');
	await selectionAction(page, 'Download original');
	expect((await download).suggestedFilename()).toMatch(/\.sdocx$/);
	await page.getByRole('checkbox', { name: 'Select all visible notes' }).check();
	await selectionAction(page, 'Delete from library');
	await page.getByRole('button', { name: 'Delete notes', exact: true }).click();
	await expect(page.locator('article.note')).toHaveCount(0);
	await page.reload();
	await expect(page.getByRole('heading', { name: 'Your notes, in one place' })).toBeVisible();
	const assets = await page.evaluate(async () => {
		const root = await (await navigator.storage.getDirectory()).getDirectoryHandle('sdocx');
		const names = [];
		for (const kind of ['originals', 'thumbnails']) {
			const directory = await root.getDirectoryHandle(kind);
			for await (const name of (
				directory as FileSystemDirectoryHandle & { keys(): AsyncIterable<string> }
			).keys())
				names.push(name);
		}
		return names;
	});
	expect(assets).toEqual([]);
});

test('storage controls preserve originals when clearing thumbnails and confirm full deletion', async ({
	page
}) => {
	await page.locator('input[type=file]').setInputFiles([note(), note('copy.sdocx')]);
	await expect(page.locator('article.note img')).toBeVisible();
	await page.getByRole('button', { name: 'Browser storage', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: 'Browser storage' });
	await expect(dialog).toContainText('Estimated site usage');
	await page.getByRole('button', { name: 'Clear thumbnail cache' }).click();
	await expect(dialog.getByRole('status')).toContainText('Thumbnails cleared');
	await dialog.getByRole('button', { name: 'Close', exact: true }).click();
	await expect(page.locator('article.note img')).toHaveCount(0);
	await page.getByRole('button', { name: /^Open / }).click();
	await expect(page.getByRole('region', { name: 'Document converter' })).toBeVisible();
	await expect(page.locator('[data-page-index] img').first()).toBeVisible();
	await page.getByRole('button', { name: 'Back to library' }).click();
	await expect(page.locator('article.note img')).toBeVisible();
	await page.getByRole('button', { name: 'Browser storage', exact: true }).click();
	await page.getByRole('button', { name: 'Delete library…', exact: true }).click();
	await expect(page.locator('article.note')).toHaveCount(1);
	await page.getByRole('button', { name: 'Keep library' }).click();
	await expect(page.getByRole('button', { name: 'Delete entire library' })).toHaveCount(0);
	await page.getByRole('button', { name: 'Delete library…', exact: true }).click();
	await page.getByRole('button', { name: 'Delete entire library' }).click();
	await expect(dialog.getByRole('status')).toHaveText('Library deleted.');
	await dialog.getByRole('button', { name: 'Close', exact: true }).click();
	await page.reload();
	await expect(page.locator('article.note')).toHaveCount(0);
});

test('quota failure offers unsaved viewing without publishing a library entry', async ({
	page
}) => {
	await page.evaluate(() => {
		FileSystemFileHandle.prototype.createWritable = async () => {
			throw new DOMException('Storage full', 'QuotaExceededError');
		};
	});
	await page.locator('input[type=file]').setInputFiles(note());
	await expect(page.locator('.import-status')).toContainText('0 imported · 1 failed');
	await expect(page.locator('article.note')).toHaveCount(0);
	await page.getByRole('button', { name: 'Open temporarily' }).click();
	await expect(page.getByRole('region', { name: 'Document converter' })).toContainText(
		'Temporary note · not saved'
	);
	await page.getByRole('button', { name: 'Back to library' }).click();
	await page.reload();
	await expect(page.locator('article.note')).toHaveCount(0);
});

test('unsupported storage retains the temporary converter workflow', async ({ page }) => {
	await page.addInitScript(() => {
		Object.defineProperty(navigator.storage, 'getDirectory', { value: undefined });
	});
	await page.reload();
	await expect(page.getByRole('alert')).toContainText('Local storage is unavailable');
	await page.locator('input[type=file]').setInputFiles(note());
	await page.getByRole('button', { name: 'Open temporarily' }).click();
	await expect(page.getByRole('region', { name: 'Document converter' })).toContainText(
		'Temporary note · not saved'
	);
});

test('library controls support keyboard use in both themes and mobile layouts', async ({
	page
}, testInfo) => {
	await page.locator('input[type=file]').setInputFiles([note(), note('second.sdocx', true)]);
	await expect(page.locator('article.note')).toHaveCount(2);
	await expect(page.getByRole('button', { name: 'Cancel import' })).toHaveCount(0);
	await page.getByRole('button', { name: 'Dismiss import results' }).click();
	const firstSelection = page.locator('article.note [role=checkbox]').first();
	await firstSelection.focus();
	await page.keyboard.press('Space');
	await expect(firstSelection).toBeChecked();
	await page.getByRole('button', { name: 'Clear selection' }).click();
	await page.screenshot({ path: testInfo.outputPath('library-dark.png') });
	await page.getByRole('button', { name: 'Use light theme' }).click();
	await page.screenshot({ path: testInfo.outputPath('library-light.png') });
	await page.setViewportSize({ width: 390, height: 844 });
	await page.screenshot({ path: testInfo.outputPath('library-mobile.png') });
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});
