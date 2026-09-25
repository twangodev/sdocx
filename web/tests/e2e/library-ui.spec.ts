import { expect, test } from '@playwright/test';
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
	await page.setViewportSize({ width: 390, height: 844 });
	expect(
		await page.locator('.library-toolbar').evaluate((el) => el.getBoundingClientRect().height)
	).toBeLessThanOrEqual(56);
	expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
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
