import { expect, test } from '../fixtures/browser';
import { pdfNote } from '../fixtures/pdf-note';

const notes = [
	{ name: 'Algebra.sdocx', mimeType: 'application/zip', buffer: pdfNote() },
	{ name: 'Biology.sdocx', mimeType: 'application/zip', buffer: pdfNote(false, true) }
];

test.beforeEach(async ({ page }) => {
	await page.route('https://rybbit.twango.dev/**', (route) => route.fulfill({ body: '' }));
	await page.goto('/');
	await expect(page.getByRole('button', { name: 'Import notes', exact: true })).toBeEnabled();
});

test('empty library offers one primary action without inactive browsing controls', async ({
	page
}) => {
	await expect(page.getByRole('button', { name: 'Import notes', exact: true })).toHaveCount(1);
	await expect(page.getByRole('searchbox')).toHaveCount(0);
	await expect(page.getByRole('checkbox')).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Sort notes' })).toHaveCount(0);
});

test('selection stays compact, has an indeterminate state, and restores browsing controls', async ({
	page
}) => {
	await page.locator('input[type=file]').setInputFiles(notes);
	await expect(page.locator('article.note')).toHaveCount(2);
	const first = page.locator('article.note').first().getByRole('checkbox');
	await first.focus();
	await page.keyboard.press('Space');
	await expect(first).toBeChecked();
	await expect(page.getByRole('checkbox', { name: 'Select all visible notes' })).toHaveAttribute(
		'aria-checked',
		'mixed'
	);
	await expect(page.getByRole('searchbox')).toHaveCount(0);
	expect(
		await page.locator('.library-toolbar').evaluate((el) => el.getBoundingClientRect().height)
	).toBeLessThanOrEqual(56);
	await page.setViewportSize({ width: 320, height: 844 });
	expect(
		await page.locator('.library-toolbar').evaluate((el) => el.getBoundingClientRect().height)
	).toBeLessThanOrEqual(56);
	expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(320);
	await page.getByRole('button', { name: 'Clear selection' }).click();
	await expect(page.getByRole('searchbox', { name: 'Search notes' })).toBeVisible();
});

test('note menus act on their own note without changing an existing selection', async ({
	page
}) => {
	await page.locator('input[type=file]').setInputFiles(notes);
	const algebra = page.getByRole('article', { name: 'Algebra.sdocx' });
	const biology = page.getByRole('article', { name: 'Biology.sdocx' });
	await algebra.getByRole('checkbox').focus();
	await page.keyboard.press('Space');
	await biology.hover();
	await biology.getByRole('button', { name: 'Actions for Biology.sdocx' }).click();
	await page.getByRole('menuitem', { name: 'Favorite', exact: true }).click();
	await expect(biology.locator('[aria-label=Favorite]')).toBeVisible();
	await expect(algebra.getByRole('checkbox')).toBeChecked();
	await biology.getByRole('button', { name: 'Actions for Biology.sdocx' }).click();
	await page.getByRole('menuitem', { name: 'Delete from library', exact: true }).click();
	await page.getByRole('button', { name: 'Delete notes', exact: true }).click();
	await expect(biology).toHaveCount(0);
	await expect(algebra.getByRole('checkbox')).toBeChecked();
});

test('mobile drawer traps focus, closes with Escape, and returns focus to its trigger', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 844 });
	const trigger = page.getByRole('button', { name: 'Library navigation', exact: true });
	await trigger.click();
	const drawer = page.getByRole('dialog', { name: 'Your library', exact: true });
	await expect(drawer).toBeVisible();
	for (let index = 0; index < 12; index++) {
		await page.keyboard.press('Tab');
		expect(await drawer.evaluate((el) => el.contains(document.activeElement))).toBe(true);
	}
	await page.keyboard.press('Escape');
	await expect(drawer).toHaveCount(0);
	await expect(trigger).toBeFocused();
	await trigger.click();
	await page.setViewportSize({ width: 1280, height: 800 });
	await expect(drawer).toHaveCount(0);
});

test('sort menu and list rows remain usable at narrow widths', async ({ page }) => {
	await page.locator('input[type=file]').setInputFiles(notes);
	await expect(page.locator('article.note')).toHaveCount(2);
	await page.getByRole('button', { name: 'Sort notes', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Title', exact: true }).click();
	await expect(page.locator('article.note').first()).toHaveAttribute('aria-label', 'Algebra.sdocx');
	await page.getByRole('radio', { name: 'List view' }).click();
	expect(
		await page
			.locator('article.note')
			.first()
			.evaluate((el) => el.getBoundingClientRect().height)
	).toBe(56);
	await page.setViewportSize({ width: 390, height: 844 });
	await expect(page.getByRole('button', { name: 'Actions for Algebra.sdocx' })).toBeVisible();
	expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
});

test('storage waits for its estimate instead of reporting unavailable while loading', async ({
	page
}) => {
	await page.evaluate(() => {
		Object.defineProperty(StorageManager.prototype, 'estimate', {
			value: () =>
				new Promise<StorageEstimate>((resolve) => {
					Object.assign(window, {
						resolveStorageEstimate: () => resolve({ usage: 1048576, quota: 104857600 })
					});
				})
		});
	});
	await page.requestGC();
	await page.getByRole('button', { name: 'Browser storage', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: 'Browser storage', exact: true });
	await expect(dialog.locator('dl')).toHaveAttribute('aria-busy', 'true');
	await expect(dialog).not.toContainText('Unavailable');
	await expect(dialog.getByRole('button', { name: 'Request persistent storage' })).toBeDisabled();
	await page.evaluate(() =>
		(window as unknown as { resolveStorageEstimate: () => void }).resolveStorageEstimate()
	);
	await expect(dialog.locator('dl')).toHaveAttribute('aria-busy', 'false');
	await expect(dialog).toContainText('1.0 MiB');
	await expect(dialog).toContainText('100.0 MiB');
	await page.keyboard.press('Escape');
	await expect(dialog).toHaveCount(0);
});

test.describe('touch controls', () => {
	test.use({ viewport: { width: 390, height: 844 }, hasTouch: true });
	test('note menus are visible without hover and support touch selection', async ({ page }) => {
		await page.locator('input[type=file]').setInputFiles(notes);
		const note = page.getByRole('article', { name: 'Algebra.sdocx' });
		const menu = note.getByRole('button', { name: 'Actions for Algebra.sdocx' });
		await expect(menu).toBeVisible();
		expect(
			await menu.evaluate((element) => getComputedStyle(element.closest('.secondary')!).opacity)
		).toBe('1');
		await menu.tap();
		await page.getByRole('menuitem', { name: 'Favorite', exact: true }).tap();
		await expect(note.locator('[aria-label=Favorite]')).toBeVisible();
		await note.getByRole('checkbox').tap();
		await expect(note.getByRole('checkbox')).toBeChecked();
		await expect(
			page.getByRole('button', { name: 'Selection actions', exact: true })
		).toBeVisible();
	});
});

test('logo returns from a document to all notes without reloading', async ({ page }) => {
	await page.locator('input[type=file]').setInputFiles(notes);
	await page.getByRole('button', { name: 'Open Algebra.sdocx', exact: true }).click();
	await expect(page.getByRole('region', { name: 'Document converter' })).toBeVisible();
	await page.evaluate(() => {
		(window as Window & { homeMarker?: boolean }).homeMarker = true;
	});
	await page.getByRole('link', { name: 'sdocx home' }).focus();
	await page.keyboard.press('Enter');
	await expect(page.getByRole('region', { name: 'Notes library' })).toBeVisible();
	await expect(page.locator('article.note')).toHaveCount(2);
	expect(await page.evaluate(() => (window as Window & { homeMarker?: boolean }).homeMarker)).toBe(
		true
	);
	await page.getByRole('button', { name: 'Open Algebra.sdocx', exact: true }).click();
	await expect(page.getByRole('region', { name: 'Document converter' })).toBeVisible();
});

test('home clears filters and empty searches have a recovery action', async ({ page }) => {
	await page.locator('input[type=file]').setInputFiles(notes);
	const search = page.getByRole('searchbox', { name: 'Search notes' });
	await search.fill('missing');
	await expect(page.getByRole('heading', { name: 'No matching notes' })).toBeVisible();
	await page.getByRole('button', { name: 'Clear search', exact: true }).first().click();
	await expect(search).toBeFocused();
	await expect(page.locator('article.note')).toHaveCount(2);
	await search.fill('missing');
	await search.press('Escape');
	await expect(search).toHaveValue('');
	await page.getByRole('button', { name: 'Favorites 0', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'No favorites yet' })).toBeVisible();
	await search.fill('Algebra');
	await page.getByRole('link', { name: 'sdocx home' }).click();
	await expect(search).toHaveValue('');
	await expect(page.getByRole('heading', { name: 'All notes', exact: true })).toBeVisible();
	await expect(page.locator('article.note')).toHaveCount(2);
});
