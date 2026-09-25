import { expect, test, type Page } from '../fixtures/browser';
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
		await expect(page.getByRole('radio', { name: 'All pages · 2' })).toBeChecked();
		if (scope === 'current') await page.getByRole('radio', { name: 'Current page · 2' }).check();
		await expect(page.getByLabel('PNG resolution')).toHaveCount(0);
		const started = page.waitForEvent('download');
		await page.getByRole('button', { name: 'Download PDF', exact: true }).click();
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

		await expect(page.getByRole('dialog').getByText('Download started', { exact: true })).toBeVisible();
		expect(remote).toEqual([]);
	});
}

test('PDF reports unsupported page dimensions without claiming a download', async ({ page }) => {
	await openDocument(page, true);
	await page.getByRole('button', { name: 'Export document', exact: true }).click();
	await page.getByLabel('Format', { exact: true }).selectOption('pdf');
	await page.getByRole('button', { name: 'Download PDF', exact: true }).click();
	await expect(page.getByRole('dialog').getByRole('alert')).toContainText('PDF dimensions');
	await expect(page.getByRole('dialog').getByText('Download started', { exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Download PDF', exact: true })).toBeEnabled();
	await page.getByRole('radio', { name: 'Custom range' }).check();
	await page.getByRole('textbox', { name: 'Page range' }).fill('2');
	await expect(page.getByRole('button', { name: 'Download PDF', exact: true })).toBeEnabled();
	const started = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Download PDF', exact: true }).click();
	expect((await started).suggestedFilename()).toBe('vector-note-page-002.pdf');
	await expect(page.getByRole('dialog').getByText('Download started', { exact: true })).toBeVisible();
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
	await expect(page.getByRole('radio', { name: 'All pages · 5' })).toBeChecked();
	const started = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Download PDF', exact: true }).click();
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
			const selection = Array.from(session.resolve_pages('2, 1–2, 1'));
			const invalidRange = message(() => session.resolve_pages('0'));
			const invalidSubset = message(() => session.render_pdf_pages(new Uint32Array([2]), 'auto'));
			const invalidPage = message(() => session.render_pdf(2, 'auto'));
			const invalidMode = message(() => session.render_pdf(undefined, 'invalid'));
			const invalidFont = message(() => session.add_pdf_font(new Uint8Array([1, 2, 3])));
			session.dispose();
			return { selection, invalidRange, invalidSubset, invalidPage, invalidMode, invalidFont, disposed: message(() => session.render_pdf(undefined, 'auto')) };
		} finally { session.free(); }
	}, [...pdfNote()]);
	expect(errors.selection).toEqual([0, 1]);
	expect(errors.invalidRange).toContain('outside');
	expect(errors.invalidSubset).toContain('out of bounds');
	expect(errors.invalidPage).toContain('out of bounds');
	expect(errors.invalidMode).toContain('color mode');
	expect(errors.invalidFont).toContain('no usable PDF font');
	expect(errors.disposed).toContain('disposed');
});

test('custom ranges validate, retain scope and package original page numbers', async ({ page }) => {
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles({ name: 'ranges.sdocx', mimeType: 'application/zip', buffer: pdfNote(false, true) });
	await expect(page.getByAltText('Rendered preview of page 3')).toBeAttached();
	await page.getByRole('button', { name: 'Export document', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(page.getByLabel('Format', { exact: true })).toHaveValue('pdf');
	await expect(page.getByRole('radio', { name: 'All pages · 3' })).toBeChecked();
	await page.getByRole('radio', { name: 'Custom range' }).check();
	const range = page.getByRole('textbox', { name: 'Page range' });
	for (const value of ['0', '3-1', '4', '1,', '999999999999999999999999']) {
		await range.fill(value);
		await expect(range).toHaveAttribute('aria-invalid', 'true');
		await expect(dialog.getByRole('button', { name: 'Download PDF', exact: true })).toBeDisabled();
	}
	await range.fill('3, 1–1, 3');
	await expect(dialog.locator('[data-export-filename]')).toHaveText('ranges-selected.pdf');
	const started = page.waitForEvent('download');
	await dialog.getByRole('button', { name: 'Download PDF', exact: true }).click();
	const download = await started;
	const { pdf } = await inspectPdf(await readFile((await download.path())!));
	expect(pdf.getPages().map((page) => page.getSize())).toEqual([{ width: 300, height: 600 }, { width: 450, height: 450 }]);
	await expect(dialog.getByText('Download started', { exact: true })).toBeVisible();
	await expect(dialog.getByRole('link', { name: 'Star on GitHub' })).not.toBeFocused();
	await page.getByLabel('Format', { exact: true }).selectOption('svg');
	await expect(range).toHaveValue('3, 1–1, 3');
	await expect(dialog.getByText('Download started', { exact: true })).toHaveCount(0);
	await expect(dialog.locator('[data-export-filename]')).toHaveText('ranges-selected-svg.zip');
	const zipped = page.waitForEvent('download');
	await dialog.getByRole('button', { name: 'Download ZIP', exact: true }).click();
	const archive = await zipped;
	const { unzipSync } = await import('fflate');
	expect(Object.keys(unzipSync(await readFile((await archive.path())!)))).toEqual(['ranges-page-001.svg', 'ranges-page-003.svg']);
	await page.getByLabel('Format', { exact: true }).selectOption('json');
	await expect(page.getByRole('radio', { name: 'Custom range' })).toHaveCount(0);
	await expect(page.getByLabel('PNG resolution')).toHaveCount(0);
	await expect(dialog.locator('[data-export-filename]')).toHaveText('ranges.json');
	await page.getByLabel('Format', { exact: true }).selectOption('everything');
	await expect(page.getByLabel('PNG resolution')).toHaveValue('1');
	await page.getByLabel('PNG resolution').selectOption('2');
	await dialog.getByRole('button', { name: 'Close', exact: true }).click();
	await page.getByRole('button', { name: 'Export document', exact: true }).click();
	await expect(page.getByLabel('Format', { exact: true })).toHaveValue('pdf');
	await expect(page.getByRole('radio', { name: 'All pages · 3' })).toBeChecked();
	await page.getByLabel('Format', { exact: true }).selectOption('png');
	await expect(page.getByLabel('PNG resolution')).toHaveValue('1');
	await page.setViewportSize({ width: 390, height: 844 });
	await expect(dialog).toBeVisible();
	const bounds = await dialog.boundingBox();
	expect(bounds!.x).toBeGreaterThanOrEqual(0);
	expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(390);
	await page.screenshot({ path: '/tmp/sdocx-export-mobile.png' });
	await page.getByRole('radio', { name: 'Current page · 1' }).check();
	await page.getByLabel('PNG resolution').selectOption('2');
	const pngStarted = page.waitForEvent('download');
	await dialog.getByRole('button', { name: 'Download PNG', exact: true }).click();
	const pngDownload = await pngStarted;
	expect(pngDownload.suggestedFilename()).toBe('ranges-page-001.png');
	const png = await readFile((await pngDownload.path())!);
	expect(png.subarray(0, 8)).toEqual(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
	expect([png.readUInt32BE(16), png.readUInt32BE(20)]).toEqual([800, 1600]);
	await page.setViewportSize({ width: 1280, height: 800 });
	await page.getByLabel('Format', { exact: true }).selectOption('pdf');
	await page.getByRole('radio', { name: 'All pages · 3' }).check();
	await page.screenshot({ path: '/tmp/sdocx-export-desktop.png' });
	await page.keyboard.press('Escape');
	await expect(dialog).not.toBeVisible();
	await expect(page.getByRole('button', { name: 'Export document', exact: true })).toBeFocused();
});

test('hiding a busy export keeps the download running', async ({ page }) => {
	await openDocument(page);
	let release!: () => void;
	const gate = new Promise<void>((resolve) => { release = resolve; });
	await page.route('**/pdf-fonts/*.ttf', async (route) => { await gate; await route.continue(); });
	await page.getByRole('button', { name: 'Export document', exact: true }).click();
	const started = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Download PDF', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('status')).toContainText('Generating PDF');
	await expect(page.getByLabel('Format', { exact: true })).toBeDisabled();
	await page.keyboard.press('Escape');
	await expect(dialog).not.toBeVisible();
	release();
	expect((await started).suggestedFilename()).toBe('vector-note.pdf');
	await expect(page.getByRole('button', { name: 'Export document', exact: true })).toBeEnabled();
});
