import { expect, test, type Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { inflateSync } from 'node:zlib';

// Replace only the parser boundary: exercise the production worker, export UI,
// SVG conversion, and downloaded PDF without requiring the external corpus.
async function openDocument(page: Page, oversized = false) {
	await page.route('https://rybbit.twango.dev/api/script.js', (route) => route.fulfill({ body: '' }));
	await page.route('**/wasm/sdocx_wasm.js', (route) => route.fulfill({
		contentType: 'application/javascript',
		body: `export default async function() {}
			export class DocumentSession {
				page_count = 2;
				inspection = {};
				render_svg(index, mode) {
					const width = ${oversized} ? 20000 : (index === 0 ? 400 : 800);
					const height = index === 0 ? 800 : 400;
					const ink = mode === 'dark' ? '#ffffff' : '#000000';
					return '<svg xmlns="http://www.w3.org/2000/svg" width="' + width + '" height="' + height + '" viewBox="0 0 ' + width + ' ' + height + '"><rect width="100%" height="100%" fill="' + (mode === 'dark' ? '#000000' : '#ffffff') + '"/><path d="M 10 20 L 100 120 L 200 20 Z" fill="' + ink + '"/><path d="M 30 60 Q 80 10 120 60" fill="none" stroke="#ff0000" stroke-width="4" stroke-opacity="0.4"/><text x="20" y="200" font-family="Arial" font-size="20">Page ' + (index + 1) + '</text></svg>';
				}
				dispose() {}
			}`
	}));
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles({ name: 'vector-note.sdocx', mimeType: 'application/zip', buffer: Buffer.from('fixture') });
	await expect(page.getByAltText('Rendered preview of page 2')).toBeAttached();
	await expect(page.getByRole('button', { name: 'Export document', exact: true })).toBeEnabled();
}

function contentStreams(pdf: Buffer): string {
	const text = pdf.toString('latin1');
	return [...text.matchAll(/stream\r?\n([\s\S]*?)\r?\nendstream/g)].map((match) =>
		inflateSync(Buffer.from(match[1], 'latin1')).toString('latin1')
	).join('\n');
}

for (const scope of ['current', 'all'] as const) {
	test(`PDF exports ${scope} pages with vector paths and original dimensions`, async ({ page }, testInfo) => {
		test.skip(testInfo.project.name !== 'chromium', 'Worker request interception requires Chromium.');
		const remote: string[] = [];
		page.on('request', (request) => {
			if (request.url().startsWith('http') && !request.url().startsWith('http://127.0.0.1:4173') && !request.url().includes('rybbit.twango.dev')) remote.push(request.url());
		});
		await openDocument(page);
		await page.getByRole('radio', { name: 'Dark document mode' }).click();
		await page.getByRole('button', { name: 'Next page' }).click();
		await expect(page.getByRole('textbox', { name: 'Page number' })).toHaveValue('2');
		await page.getByRole('button', { name: 'Export document', exact: true }).click();
		await page.getByLabel('Format', { exact: true }).selectOption('pdf');
		await page.getByLabel('Pages', { exact: true }).selectOption(scope);
		await expect(page.getByLabel('PNG resolution')).toHaveCount(0);
		const started = page.waitForEvent('download');
		await page.getByRole('button', { name: 'Download', exact: true }).click();
		const download = await started;
		expect(download.suggestedFilename()).toBe(scope === 'all' ? 'vector-note.pdf' : 'vector-note-page-002.pdf');
		const pdf = await readFile((await download.path())!);
		const source = pdf.toString('latin1');
		expect(source).toMatch(/^%PDF-/);
		expect(source.match(/\/Type \/Page\b/g)).toHaveLength(scope === 'all' ? 2 : 1);
		expect(source).toContain('/MediaBox [0 0 600. 300.]');
		if (scope === 'all') expect(source).toContain('/MediaBox [0 0 300. 600.]');
		expect(source).not.toContain('/Subtype /Image');
		const contents = contentStreams(pdf);
		expect(contents).toMatch(/10\.? 20\.? m/);
		expect(contents).toMatch(/100\.? 120\.? l/);
		expect(contents).toContain('(Page 2)');
		expect(contents).toMatch(/1\.?(?:0*) g/); // White ink from document dark mode.
		expect(contents.includes('(Page 1)')).toBe(scope === 'all');
		if (scope === 'all') expect(contents.indexOf('(Page 1)')).toBeLessThan(contents.indexOf('(Page 2)'));
		await expect(page.getByText('Your download is ready.')).toBeVisible();
		expect(remote).toEqual([]);
	});
}

test('PDF reports unsupported page dimensions without claiming a download', async ({ page }, testInfo) => {
	test.skip(testInfo.project.name !== 'chromium', 'Worker request interception requires Chromium.');
	await openDocument(page, true);
	await page.getByRole('button', { name: 'Export document', exact: true }).click();
	await page.getByLabel('Format', { exact: true }).selectOption('pdf');
	await page.getByRole('button', { name: 'Download', exact: true }).click();
	await expect(page.getByRole('dialog').getByRole('alert')).toContainText('page dimensions');
	await expect(page.getByText('Your download is ready.')).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Download', exact: true })).toBeEnabled();
});

const realFixture = resolve('../hf/01-basic-formatting.sdocx');
test('real WASM document exports all pages as a PDF', async ({ page }) => {
	test.skip(!existsSync(realFixture), 'External corpus is not checked out.');
	await page.route('https://rybbit.twango.dev/api/script.js', (route) => route.fulfill({ body: '' }));
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles(realFixture);
	await expect(page.getByAltText('Rendered preview of page 5')).toBeAttached();
	await page.getByRole('button', { name: 'Export document', exact: true }).click();
	await page.getByLabel('Format', { exact: true }).selectOption('pdf');
	await page.getByLabel('Pages', { exact: true }).selectOption('all');
	const started = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Download', exact: true }).click();
	const download = await started;
	expect(download.suggestedFilename()).toBe('01-basic-formatting.pdf');
	const pdf = (await readFile((await download.path())!)).toString('latin1');
	expect(pdf).toMatch(/^%PDF-/);
	expect(pdf.match(/\/Type \/Page\b/g)).toHaveLength(5);
});
