import { expect, test, type Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { PDFDocument, PDFRawStream, decodePDFRawStream } from 'pdf-lib';
import { pdfNote } from '../fixtures/pdf-note';

async function openDocument(page: Page, oversized = false) {
	await page.route('https://rybbit.twango.dev/api/script.js', (route) => route.fulfill({ body: '' }));
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles({ name: 'vector-note.sdocx', mimeType: 'application/zip', buffer: pdfNote(oversized) });
	await expect(page.getByAltText('Rendered preview of page 2')).toBeAttached();
	await expect(page.getByRole('button', { name: 'Export document', exact: true })).toBeEnabled();
}

async function inspectPdf(bytes: Buffer) {
	const pdf = await PDFDocument.load(bytes);
	const objects = pdf.context.enumerateIndirectObjects().map(([, value]) => value);
	const contents = objects.filter((value): value is PDFRawStream => value instanceof PDFRawStream)
		.map((stream) => Buffer.from(decodePDFRawStream(stream).decode()).toString('latin1')).join('\n');
	return { pdf, contents, dictionaries: objects.map((value) => value instanceof PDFRawStream ? value.dict.toString() : value.toString()).join('\n') };
}

for (const scope of ['current', 'all'] as const) {
	test(`PDF exports ${scope} pages with vector paths and original dimensions`, async ({ page }) => {
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
		const bytes = await readFile((await download.path())!);
		expect(bytes.toString('latin1')).toMatch(/^%PDF-/);
		const { pdf, contents, dictionaries } = await inspectPdf(bytes);
		expect(pdf.getPageCount()).toBe(scope === 'all' ? 2 : 1);
		expect(pdf.getPages().map((page) => page.getSize())).toEqual(scope === 'all'
			? [{ width: 300, height: 600 }, { width: 600, height: 300 }]
			: [{ width: 600, height: 300 }]);
		expect(dictionaries).not.toContain('/Subtype /Image');
		expect(dictionaries).toContain('/FontFile2');
		expect(dictionaries).toContain('/ToUnicode');
		expect(contents).toContain('30 40 m 150 80 l');
		expect(contents.includes('10 20 m 100 120 l')).toBe(scope === 'all');
		expect(contents).toContain('<03A9>'); // Unicode mapping for Ω.
		expect(contents).toContain('<00E9>'); // Unicode mapping for é.
		expect(contents).toMatch(/\b1 G\b/); // White stroke from document dark mode.

		await expect(page.getByText('Your download is ready.')).toBeVisible();
		expect(remote).toEqual([]);
	});
}

test('PDF reports unsupported page dimensions without claiming a download', async ({ page }) => {
	await openDocument(page, true);
	await page.getByRole('button', { name: 'Export document', exact: true }).click();
	await page.getByLabel('Format', { exact: true }).selectOption('pdf');
	await page.getByRole('button', { name: 'Download', exact: true }).click();
	await expect(page.getByRole('dialog').getByRole('alert')).toContainText('PDF dimensions');
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
	const bytes = await readFile((await download.path())!);
	const { pdf } = await inspectPdf(bytes);
	expect(pdf.getPageCount()).toBe(5);
});

test('WASM PDF API validates page, color, fonts and disposed sessions', async ({ page }) => {
	await page.route('https://rybbit.twango.dev/api/script.js', (route) => route.fulfill({ body: '' }));
	await page.goto('/');
	const errors = await page.evaluate(async (bytes) => {
		const module = await import(`${location.origin}/wasm/sdocx_wasm.js`);
		await module.default();
		const session = new module.DocumentSession(new Uint8Array(bytes));
		const message = (task: () => unknown) => {
			try { task(); return ''; } catch (error) { return (error as Error).message; }
		};
		try {
			const invalidPage = message(() => session.render_pdf(2, 'auto'));
			const invalidMode = message(() => session.render_pdf(undefined, 'invalid'));
			const invalidFont = message(() => session.add_pdf_font(new Uint8Array([1, 2, 3])));
			session.dispose();
			return { invalidPage, invalidMode, invalidFont, disposed: message(() => session.render_pdf(undefined, 'auto')) };
		} finally { session.free(); }
	}, [...pdfNote()]);
	expect(errors.invalidPage).toContain('out of bounds');
	expect(errors.invalidMode).toContain('color mode');
	expect(errors.invalidFont).toContain('no usable PDF font');
	expect(errors.disposed).toContain('disposed');
});
