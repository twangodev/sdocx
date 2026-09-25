import { expect, test } from '@playwright/test';
import { pdfNote } from '../fixtures/pdf-note';

test('WebKit private sessions offer temporary viewing when real OPFS is unavailable', async ({
	page,
	browserName
}) => {
	test.skip(browserName !== 'webkit', 'WebKit private sessions do not support OPFS.');
	await page.route('https://rybbit.twango.dev/**', (route) => route.fulfill({ body: '' }));
	await page.goto('/');
	await expect(page.getByRole('alert')).toBeVisible();
	await page.locator('input[type=file]').setInputFiles({
		name: 'temporary.sdocx',
		mimeType: 'application/zip',
		buffer: pdfNote()
	});
	await page.getByRole('button', { name: 'Open temporarily' }).click();
	await expect(page.getByRole('region', { name: 'Document converter' })).toContainText(
		'Temporary note · not saved'
	);
	await expect(page.getByAltText('Rendered preview of page 1')).toBeAttached();
	await page.getByRole('link', { name: 'sdocx home' }).click();
	await page.reload();
	await expect(page.getByRole('button', { name: 'Import notes', exact: true })).toBeEnabled();
	await expect(page.locator('article.note')).toHaveCount(0);
});
