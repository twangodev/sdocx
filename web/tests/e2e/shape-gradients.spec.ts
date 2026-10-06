import { expect, test } from '../fixtures/browser';
import { readFile } from 'node:fs/promises';
import { PDFDocument, PDFRawStream, decodePDFRawStream } from 'pdf-lib';
import { shapeGradientNote } from '../fixtures/pdf-note';

test('parsed shape gradients reach the worker preview and downloaded vector PDF', async ({ page, browserName }) => {
	test.skip(browserName !== 'chromium', 'Chromium preview and vector exports are the immediate target.');
	await page.route('https://rybbit.twango.dev/api/script.js', route => route.fulfill({ body: '' }));
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles({ name: 'shape-gradients.sdocx', mimeType: 'application/zip', buffer: shapeGradientNote() });
	const preview = page.getByAltText('Rendered preview of page 1');
	await expect(preview).toBeAttached();
	await expect(page.getByRole('button', { name: 'Export document', exact: true })).toBeEnabled();
	const paints = await preview.evaluate(async (image: HTMLImageElement) => {
		const xml = new DOMParser().parseFromString(await (await fetch(image.src)).text(), 'image/svg+xml');
		const gradient = (tag: 'linearGradient' | 'radialGradient', role: 'fill' | 'stroke') => {
			const definition = xml.querySelector(tag);
			if (!definition) return null;
			const reference = `url(#${definition.id})`;
			const paint = [...xml.querySelectorAll(`[${role}]`)].find(node => node.getAttribute(role) === reference);
			return {
				units: definition.getAttribute('gradientUnits'),
				drawsShape: Boolean(paint?.matches('rect, ellipse, polygon, path') || paint?.querySelector('rect, ellipse, polygon, path')),
				stops: [...definition.querySelectorAll('stop')].map(stop => ({
					offset: Number(stop.getAttribute('offset')),
					color: stop.getAttribute('stop-color'),
					opacity: Number(stop.getAttribute('stop-opacity') ?? 1)
				}))
			};
		};
		const survivingOutline = [...xml.querySelectorAll('[stroke]')].find(node => node.getAttribute('stroke') === '#ff00ff');
		return {
			definitions: xml.querySelectorAll('linearGradient, radialGradient').length,
			linearFill: gradient('linearGradient', 'fill'), radialOutline: gradient('radialGradient', 'stroke'),
			rejectedFill: survivingOutline?.getAttribute('fill'),
			survivingOutlineDrawsShape: Boolean(survivingOutline?.matches('rect') || survivingOutline?.querySelector('rect')),
			images: xml.querySelectorAll('image').length
		};
	});
	expect(paints.definitions).toBe(2);
	expect(paints.images).toBe(0);
	expect(paints.linearFill).toMatchObject({ units: 'userSpaceOnUse', drawsShape: true,
		stops: [{ offset: 0, color: '#ff0000' }, { offset: 1, color: '#0000ff' }] });
	expect(paints.radialOutline).toMatchObject({ units: 'userSpaceOnUse', drawsShape: true,
		stops: [{ offset: 0, color: '#00ff00' }, { offset: 1, color: '#ffff00' }] });
	expect(paints.linearFill!.stops[0].opacity).toBeCloseTo(128 / 255, 4);
	expect(paints.linearFill!.stops[1].opacity).toBeCloseTo(192 / 255, 4);
	expect(paints.radialOutline!.stops[0].opacity).toBeCloseTo(64 / 255, 4);
	expect(paints.radialOutline!.stops[1].opacity).toBe(1);
	expect(paints.rejectedFill).toBe('none');
	expect(paints.survivingOutlineDrawsShape).toBe(true);

	const omission = /Fill · Invalid Gradient · object gradient-rejected-fill · source byte \d+/;
	const info = page.getByRole('complementary', { name: 'Document information' });
	await info.locator('summary', { hasText: 'Preview rendering (auto)' }).click();
	await expect(info.getByText(omission)).toBeVisible();
	await page.getByRole('button', { name: 'Export document', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await page.getByLabel('Format', { exact: true }).selectOption('pdf');
	await dialog.locator('summary', { hasText: 'Page preview rendering (auto)' }).click();
	await expect(dialog.getByText(omission)).toBeVisible();
	const started = page.waitForEvent('download');
	await dialog.getByRole('button', { name: 'Download PDF', exact: true }).click();
	const download = await started;
	expect(download.suggestedFilename()).toBe('shape-gradients.pdf');
	const pdf = await PDFDocument.load(await readFile((await download.path())!));
	expect(pdf.getPageCount()).toBe(1);
	const objects = pdf.context.enumerateIndirectObjects().map(([, value]) => value);
	const dictionaries = objects.map(value => value instanceof PDFRawStream ? value.dict.toString() : value.toString()).join('\n');
	expect(dictionaries).toMatch(/\/ShadingType 2\b/);
	expect(dictionaries).toMatch(/\/ShadingType 3\b/);
	expect(dictionaries).not.toContain('/Subtype /Image');
	const contents = objects.filter((value): value is PDFRawStream => value instanceof PDFRawStream)
		.map(stream => Buffer.from(decodePDFRawStream(stream).decode()).toString('latin1')).join('\n');
	expect(contents, 'PDF must select a gradient fill pattern').toMatch(/\/Pattern cs\s+\/[^\s/]+ scn\b/);
	expect(contents, 'PDF must select a gradient outline pattern').toMatch(/\/Pattern CS\s+\/[^\s/]+ SCN\b/);
	expect([...contents.matchAll(/\/[^\s/]+\s+sh\b/g)].length, 'PDF must draw both gradient alpha shaders').toBeGreaterThanOrEqual(2);
	await dialog.locator('summary', { hasText: 'Download rendering (auto)' }).click();
	await expect(dialog.getByText(omission)).toHaveCount(2);
	await expect(dialog.getByText('Download started', { exact: true })).toBeVisible();
});
