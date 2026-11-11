import { expect, test } from '../fixtures/browser';
import { pdfNote } from '../fixtures/pdf-note';

test('retries a failed binary initialization when reopening a saved note in the same worker', async ({ page }) => {
	await page.route('https://rybbit.twango.dev/**', route => route.fulfill({ body: '' }));
	await page.addInitScript(() => {
		const NativeWorker = window.Worker;
		Object.assign(window, { initializationTestWorkers: 0 });
		window.Worker = class extends NativeWorker {
			constructor(url: string | URL, options?: WorkerOptions) {
				super(url, options);
				Reflect.set(window, 'initializationTestWorkers', Reflect.get(window, 'initializationTestWorkers') + 1);
			}
		};
	});
	await page.goto('/');
	await expect(page.getByRole('button', { name: 'Import notes', exact: true })).toBeEnabled();
	const note = { name: 'initialization.sdocx', mimeType: 'application/zip', buffer: pdfNote() };
	await page.locator('input[type=file]').setInputFiles([note, { ...note, name: 'duplicate.sdocx' }]);
	await expect(page.locator('article.note')).toHaveCount(1);
	await expect(page.getByRole('button', { name: 'Cancel import' })).toHaveCount(0);
	const workersBeforeOpening = await page.evaluate(() => Reflect.get(window, 'initializationTestWorkers'));
	let binaryRequests = 0;
	await page.route('**/wasm/sdocx_wasm_bg.wasm', async route => {
		binaryRequests += 1;
		if (binaryRequests === 1) {
			await route.fulfill({ status: 503, contentType: 'application/wasm', body: 'Temporarily unavailable' });
		} else {
			await route.continue();
		}
	});
	await page.getByRole('button', { name: /^Open initialization/ }).click();
	await expect(page.getByRole('alert')).toBeVisible();
	await expect(page.getByRole('region', { name: 'Document converter' })).toHaveCount(0);
	expect(binaryRequests).toBe(1);
	await page.getByRole('button', { name: /^Open initialization/ }).click();
	expect(await page.evaluate(() => Reflect.get(window, 'initializationTestWorkers'))).toBe(workersBeforeOpening);
	await expect.poll(() => binaryRequests).toBe(2);
	await expect(page.getByRole('region', { name: 'Document converter' })).toBeVisible();
	await expect(page.getByAltText('Rendered preview of page 1')).toBeVisible();
	expect(binaryRequests).toBe(2);
	expect(await page.evaluate(() => Reflect.get(window, 'initializationTestWorkers'))).toBe(workersBeforeOpening);
	await page.getByRole('button', { name: 'Back to library', exact: true }).click();
	await page.getByRole('button', { name: /^Open initialization/ }).click();
	await expect(page.getByAltText('Rendered preview of page 1')).toBeVisible();
	expect(binaryRequests).toBe(2);
	expect(await page.evaluate(() => Reflect.get(window, 'initializationTestWorkers'))).toBe(workersBeforeOpening);
});
