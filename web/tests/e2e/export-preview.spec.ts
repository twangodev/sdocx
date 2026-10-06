import { test, expect, type Page } from '../fixtures/browser';
import { readFile } from 'node:fs/promises';
import { PDFDocument, PDFRawStream, decodePDFRawStream } from 'pdf-lib';
import { geometryNote, pdfNote } from '../fixtures/pdf-note';
import { unzipSync } from 'fflate';

declare global {
	interface Window {
		exportPreviewBlobs: Map<string, Blob>;
		exportPreviewTest: { hold: boolean; fail: boolean; pending: (() => void)[]; delivered: number; revoked: string[] };
	}
}

async function openDocument(page: Page) {
	await page.addInitScript(() => {
		const blobs = window.exportPreviewBlobs = new Map<string, Blob>();
		const create = URL.createObjectURL;
		URL.createObjectURL = blob => {
			const url = create(blob);
			if (blob instanceof Blob) blobs.set(url, blob);
			return url;
		};
	});
	await page.route('https://rybbit.twango.dev/api/script.js', route => route.fulfill({ body: '' }));
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles({ name: 'preview.sdocx', mimeType: 'application/zip', buffer: pdfNote(false, true) });
	await expect(page.getByAltText('Rendered preview of page 3')).toBeAttached();
	await expect(page.getByRole('button', { name: 'Export document', exact: true })).toBeEnabled();
}

async function previewColor(page: Page) {
	return page.getByRole('dialog').locator('img').evaluate(async (image: HTMLImageElement) => {
		try {
			const svg = await window.exportPreviewBlobs.get(image.src)?.text();
			if (!svg) return null;
			return new DOMParser().parseFromString(svg, 'image/svg+xml').querySelector('rect')?.getAttribute('fill');
		} catch { return null; }
	});
}

test('detailed WASM PDF owns its bytes and ordered reports after the session is freed', async ({ page }) => {
	await openDocument(page);
	const input = geometryNote();
	const sourcePage = Buffer.from(unzipSync(input)['two2.page']);
	const sourceOffset = sourcePage.indexOf(Buffer.from('geometry-line')) - 24;
	const result = await page.evaluate(async input => {
		const wasm = await import(`${location.origin}/wasm/sdocx_wasm.js`);
		const exports = await wasm.default({ module_or_path: `${location.origin}/wasm/sdocx_wasm_bg.wasm` });
		const session = new wasm.DocumentSession(new Uint8Array(input));
		const rendered = session.render_svg_detailed(1, 'auto');
		const pdf = session.render_pdf_pages_detailed(new Uint32Array([1, 0, 1]), 'auto');
		const ownedBytes = pdf.bytes instanceof Uint8Array && pdf.bytes.buffer instanceof ArrayBuffer
			&& pdf.bytes.buffer !== exports.memory.buffer;
		const beforeBytes = Array.from(pdf.bytes);
		const before = JSON.stringify({ rendered, pages: pdf.pages });
		session.dispose();
		session.free();
		const replacement = new wasm.DocumentSession(new Uint8Array(input));
		replacement.render_pdf_pages_detailed(new Uint32Array([0]), 'dark');
		replacement.dispose();
		replacement.free();
		return {
			ownedBytes,
			bytesSurvived: JSON.stringify(Array.from(pdf.bytes)) === JSON.stringify(beforeBytes),
			signature: Array.from(pdf.bytes.subarray(0, 4)),
			pageIndices: pdf.pages.map((report: { page_index: number }) => report.page_index),
			sourceIndices: pdf.pages.map((report: { source_page_index: number }) => report.source_page_index),
			svgPage: [rendered.page_index, rendered.source_page_index],
			geometry: rendered.geometry_diagnostics,
			pdfGeometry: pdf.pages.map((report: { geometry_diagnostics: unknown[] }) => report.geometry_diagnostics),
			reportsSurvived: JSON.stringify({ rendered, pages: pdf.pages }) === before,
			svgSurvived: typeof rendered.svg === 'string' && rendered.svg.includes('<svg')
		};
	}, Array.from(input));
	const geometry = [{ kind: 'UnsupportedLineType', object_uuid: 'geometry-line', source_offset: sourceOffset }];
	expect(sourceOffset).toBeGreaterThan(0);
	expect(result).toEqual({ ownedBytes: true, bytesSurvived: true, signature: [37, 80, 68, 70], pageIndices: [1, 0, 1], sourceIndices: [1, 0, 1], svgPage: [1, 1], geometry, pdfGeometry: [geometry, [], geometry], reportsSurvived: true, svgSurvived: true });
});

test('actual geometry omissions reach viewer and download notices with object identity', async ({ page }) => {
	await page.route('https://rybbit.twango.dev/api/script.js', route => route.fulfill({ body: '' }));
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles({ name: 'geometry.sdocx', mimeType: 'application/zip', buffer: geometryNote() });
	await expect(page.getByAltText('Rendered preview of page 2')).toBeAttached();
	const info = page.getByRole('complementary', { name: 'Document information' });
	await expect(info).toBeVisible();
	await info.locator('summary', { hasText: 'Preview rendering (auto)' }).click();
	await expect(info.getByText(/Unsupported Line Type · object geometry-line · source byte \d+/)).toBeVisible();
	await page.getByRole('button', { name: 'Export document', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: 'Next export preview page' }).click();
	await dialog.locator('summary', { hasText: 'Page preview rendering (auto)' }).click();
	await expect(dialog.getByText(/Unsupported Line Type · object geometry-line · source byte \d+/)).toBeVisible();
	const download = page.waitForEvent('download');
	await dialog.getByRole('button', { name: 'Download PDF' }).click();
	await download;
	await dialog.locator('summary', { hasText: 'Download rendering (auto)' }).click();
	await expect(dialog.getByText(/Unsupported Line Type · object geometry-line · source byte \d+/)).toHaveCount(2);
});

test('rendering notices stay scoped to viewer, export preview, and download colors', async ({ page }) => {
	await page.addInitScript(() => {
		const post = Worker.prototype.postMessage;
		const patched = new WeakSet<Worker>();
		const requests = new WeakMap<Worker, Map<number, { type: string; colorMode: string }>>();
		Worker.prototype.postMessage = function(message, options?: StructuredSerializeOptions | Transferable[]) {
			if (!patched.has(this)) {
				patched.add(this);
				const pending = new Map<number, { type: string; colorMode: string }>();
				requests.set(this, pending);
				const receive = this.onmessage;
				this.onmessage = event => {
					const request = pending.get(event.data.id);
					pending.delete(event.data.id);
					if (request && event.data.type === 'result') {
						const reports = request.type === 'exportPdf' ? event.data.value.pages : [event.data.value];
						for (const report of reports) {
							report.object_diagnostics.push({ kind: `Browser${request.colorMode === 'dark' ? 'Dark' : 'Auto'}Notice`, anchor_utf16: 0 });
							if (request.colorMode === 'dark') {
								report.source_page_index += 7;
								report.geometry_diagnostics.push({ kind: 'FutureGeometryNotice', object_uuid: 'future-id', source_offset: null });
								report.paint_diagnostics.push({ kind: 'UnsupportedGradientFrame', role: 'Fill', object_uuid: 'fill-id', source_offset: 321 });
								report.paint_diagnostics.push({ kind: 'FuturePaintNotice', role: 'Outline', object_uuid: 'outline-id', source_offset: null });
							}
						}
					}
					receive?.call(this, event);
				};
			}
			if (message.type === 'renderPage' || message.type === 'exportPdf') requests.get(this)!.set(message.id, message);
			post.call(this, message, options as StructuredSerializeOptions);
		};
	});
	await openDocument(page);
	const info = page.getByRole('complementary', { name: 'Document information' });
	await expect(info).toBeVisible();
	await info.locator('summary', { hasText: 'Preview rendering (auto)' }).click();
	await expect(info.getByText('Browser Auto Notice · text position 0')).toHaveCount(3);
	await page.getByRole('button', { name: 'Export document', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('radio', { name: 'Dark document mode' }).click();
	await dialog.locator('summary', { hasText: 'Page preview rendering (dark)' }).click();
	await expect(dialog.getByText('Browser Dark Notice · text position 0')).toBeVisible();
	await expect(dialog.getByText('Future Geometry Notice · object future-id')).toBeVisible();
	await expect(dialog.getByText('Fill · Unsupported Gradient Frame · object fill-id · source byte 321')).toBeVisible();
	await expect(dialog.getByText('Outline · Future Paint Notice · object outline-id')).toBeVisible();
	await expect(dialog.getByText(/source page 8/)).toHaveCount(1);
	await dialog.getByRole('radio', { name: 'Current page · 1' }).check();
	const download = page.waitForEvent('download');
	await dialog.getByRole('button', { name: 'Download PDF' }).click();
	await download;
	await dialog.locator('summary', { hasText: 'Download rendering (dark)' }).click();
	await expect(dialog.getByText('Browser Dark Notice · text position 0')).toHaveCount(2);
	await expect(dialog.getByText('Fill · Unsupported Gradient Frame · object fill-id · source byte 321')).toHaveCount(2);
	await expect(dialog.getByText('Outline · Future Paint Notice · object outline-id')).toHaveCount(2);
	await expect(dialog.getByText(/source page 8/)).toHaveCount(2);
	await page.keyboard.press('Escape');
	await expect(info.getByText('Browser Auto Notice · text position 0')).toHaveCount(3);
	await expect(info.getByText('Browser Dark Notice · text position 0')).toHaveCount(0);
	await expect(info.getByText('Fill · Unsupported Gradient Frame · object fill-id · source byte 321')).toHaveCount(0);
	await expect(info.getByText('Outline · Future Paint Notice · object outline-id')).toHaveCount(0);
});

test('selected-page preview supports navigation, zoom, resolution, and JSON summary', async ({ page }) => {
	await openDocument(page);
	await page.getByRole('button', { name: 'Next page', exact: true }).click();
	await page.getByRole('button', { name: 'Export document', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByAltText('Export preview of page 2')).toBeVisible();
	await expect(dialog.locator('[data-export-preview-page]')).toHaveText('Page 2 · 2 of 3 selected');
	await dialog.getByRole('radio', { name: 'Custom range' }).check();
	await dialog.getByLabel('Page range', { exact: true }).fill('1, 3');
	await expect(dialog.getByAltText('Export preview of page 1')).toBeVisible();
	await dialog.getByRole('button', { name: 'Next export preview page' }).click();
	await expect(dialog.getByAltText('Export preview of page 3')).toBeVisible();
	await expect(dialog.getByRole('button', { name: 'Next export preview page' })).toBeDisabled();
	const image = dialog.getByAltText('Export preview of page 3');
	const width = (await image.boundingBox())!.width;
	await dialog.getByRole('button', { name: 'Zoom in export preview' }).click();
	await expect.poll(async () => (await image.boundingBox())!.width).toBeGreaterThan(width);
	await dialog.getByRole('button', { name: 'Fit export preview to page' }).click();
	await expect.poll(async () => (await image.boundingBox())!.width).toBeCloseTo(width, 0);
	await dialog.getByLabel('Format', { exact: true }).selectOption('png');
	await dialog.getByLabel('PNG resolution').selectOption('2');
	await expect(dialog.getByText('1200 × 1200 px output')).toBeVisible();
	await dialog.getByLabel('Page range', { exact: true }).fill('99');
	await expect(dialog.locator('img')).toHaveCount(0);
	await expect(dialog.getByRole('button', { name: /^Download / })).toBeDisabled();
	await dialog.getByLabel('Format', { exact: true }).selectOption('json');
	await expect(dialog.getByText('Document structure', { exact: true })).toBeVisible();
	await expect(dialog.locator('img')).toHaveCount(0);
	await expect(dialog.getByRole('button', { name: 'Download JSON' })).toBeEnabled();
});

test('export colors change the preview and SVG/PDF downloads without changing the viewer', async ({ page }) => {
	await openDocument(page);
	const viewer = page.getByAltText('Rendered preview of page 1');
	const original = await viewer.getAttribute('src');
	await page.getByRole('button', { name: 'Export document', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByAltText('Export preview of page 1')).toBeVisible();
	await dialog.getByRole('radio', { name: 'Dark document mode' }).click();
	await expect.poll(() => previewColor(page)).toBe('#252525');
	await dialog.getByLabel('Format', { exact: true }).selectOption('svg');
	await dialog.getByRole('radio', { name: 'Current page · 1' }).check();
	const started = page.waitForEvent('download');
	await dialog.getByRole('button', { name: 'Download SVG' }).click();
	const svg = await readFile((await (await started).path())!, 'utf8');
	expect(svg).toContain('fill="#252525"');
	expect(svg).toContain('stroke="#ffffff"');
	await dialog.getByLabel('Format', { exact: true }).selectOption('pdf');
	const pdfStarted = page.waitForEvent('download');
	await dialog.getByRole('button', { name: 'Download PDF' }).click();
	const pdf = await PDFDocument.load(await readFile((await (await pdfStarted).path())!));
	const contents = pdf.context.enumerateIndirectObjects()
		.map(([, value]) => value)
		.filter((value): value is PDFRawStream => value instanceof PDFRawStream)
		.map(stream => Buffer.from(decodePDFRawStream(stream).decode()).toString('latin1')).join('\n');
	expect(pdf.getPageCount()).toBe(1);
	expect(contents).toMatch(/\b1 G\b/);
	await page.keyboard.press('Escape');
	await expect(dialog).not.toBeVisible();
	await expect(viewer).toHaveAttribute('src', original!);
	await expect(page.getByRole('radio', { name: 'Automatic color mode' })).toBeChecked();
	await expect(page.getByRole('button', { name: 'Export document', exact: true })).toBeFocused();
});

test('preview rejects stale results, retries failures, and releases its object URLs', async ({ page }) => {
	await page.addInitScript(() => {
		const controls = window.exportPreviewTest = { hold: false, fail: false, delivered: 0, pending: [] as (() => void)[], revoked: [] as string[] };
		const post = Worker.prototype.postMessage;
		Worker.prototype.postMessage = function(message, options?: StructuredSerializeOptions | Transferable[]) {
			if (message.type === 'renderPage' && message.colorMode === 'dark') {
				if (controls.fail) {
					setTimeout(() => this.dispatchEvent(new MessageEvent('message', { data: { id: message.id, type: 'error', message: 'Preview temporarily unavailable' } })), 0);
					return;
				}
				if (controls.hold) {
					const received = (event: MessageEvent) => {
						if (event.data.id !== message.id) return;
						controls.delivered++;
						this.removeEventListener('message', received);
					};
					this.addEventListener('message', received);
					controls.pending.push(() => post.call(this, message, options as StructuredSerializeOptions)); return;
				}
			}
			post.call(this, message, options as StructuredSerializeOptions);
		};
		const revoke = URL.revokeObjectURL;
		URL.revokeObjectURL = url => { controls.revoked.push(url); revoke(url); };
	});
	await openDocument(page);
	await page.getByRole('button', { name: 'Export document', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.locator('img')).toBeVisible();
	const first = await dialog.locator('img').getAttribute('src');
	await page.evaluate(() => window.exportPreviewTest.hold = true);
	await dialog.getByRole('radio', { name: 'Dark document mode' }).click();
	await expect.poll(() => page.evaluate(() => window.exportPreviewTest.pending.length)).toBe(1);
	await dialog.getByRole('radio', { name: 'Light document mode' }).click();
	await expect.poll(() => previewColor(page)).toBe('#fcfcfc');
	await page.evaluate(() => { window.exportPreviewTest.hold = false; window.exportPreviewTest.pending.splice(0).forEach(release => release()); });
	await expect.poll(() => page.evaluate(() => window.exportPreviewTest.delivered)).toBe(1);
	await expect.poll(() => previewColor(page)).toBe('#fcfcfc');
	await expect.poll(() => page.evaluate(() => window.exportPreviewTest.revoked)).toContain(first);
	await page.evaluate(() => window.exportPreviewTest.fail = true);
	await dialog.getByRole('radio', { name: 'Dark document mode' }).click();
	await expect(dialog.getByRole('alert')).toHaveText('Preview temporarily unavailable');
	await expect(dialog.getByRole('button', { name: 'Download PDF' })).toBeEnabled();
	await page.evaluate(() => window.exportPreviewTest.fail = false);
	await dialog.getByRole('button', { name: 'Retry preview' }).click();
	await expect.poll(() => previewColor(page)).toBe('#252525');
	const last = await dialog.locator('img').getAttribute('src');
	await page.keyboard.press('Escape');
	await expect.poll(() => page.evaluate(() => window.exportPreviewTest.revoked)).toContain(last);
});

for (const theme of ['light', 'dark']) {
	test(`dialog fits desktop and mobile in the ${theme} interface theme`, async ({ page }, testInfo) => {
		await page.addInitScript(theme => localStorage.setItem('sdocx-theme', theme), theme);
		await openDocument(page);
		await page.getByRole('button', { name: 'Export document', exact: true }).click();
		const dialog = page.getByRole('dialog');
		await expect(dialog.locator('img')).toBeVisible();
		for (const viewport of [{ width: 1280, height: 800 }, { width: 390, height: 844 }, { width: 844, height: 390 }]) {
			await page.setViewportSize(viewport);
			await expect(dialog.getByRole('button', { name: 'Download PDF' })).toBeInViewport();
			expect(await dialog.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
			await page.screenshot({ path: `/tmp/export-preview-${theme}-${viewport.width}.png` });
			await testInfo.attach(`${theme}-${viewport.width}`, { body: await dialog.screenshot(), contentType: 'image/png' });
		}
	});
}
