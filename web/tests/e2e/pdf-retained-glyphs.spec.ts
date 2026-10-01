import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { PDFDocument, PDFHexString, PDFObjectParser, PDFRawStream, PDFString, decodePDFRawStream } from 'pdf-lib';
import { expect, test } from '../fixtures/browser';
import { retainedPdfNote, retainedSource } from '../fixtures/retained-pdf-note';

test('WASM PDF download retains Arabic ligature source and embedded vector fonts', async ({ page, browserName }) => {
	test.skip(browserName !== 'chromium', 'Chromium preview and vector exports are the immediate target.');
	const font = await readFile(resolve('../crates/sdocx/tests/assets/fonts/DejaVuSans.ttf'));
	expect(createHash('sha256').update(font).digest('hex')).toBe('57f73e11f51999432bf7ab22ce55b6f945d5eca1bf824404cfa9ec2e3718c84e');
	await page.route('https://rybbit.twango.dev/api/script.js', route => route.fulfill({ body: '' }));
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Your notes, in one place', exact: true })).toBeVisible();
	await page.evaluate(() => document.fonts.ready.then(() => undefined));
	const fontRequests: string[] = [];
	page.on('request', request => {
		if (/^https?:/.test(request.url()) && /\.(?:ttf|otf|woff2?)(?:\?|$)/.test(request.url())) fontRequests.push(request.url());
	});
	const result = await page.evaluate(async ({ note, font }) => {
		const module = await import(`${location.origin}/wasm/sdocx_wasm.js`);
		await module.default();
		const session = new module.DocumentSession(new Uint8Array(note));
		try {
			session.add_pdf_font(new Uint8Array(font));
			const storedSource = session.inspection().document.pages[0].objects[0].content.Element.TextBox.text;
			const svg = session.render_svg(0, 'light');
			const parsed = new DOMParser().parseFromString(svg, 'image/svg+xml');
			const styles = [...parsed.querySelectorAll('style')].map(style => style.textContent).join('\n');
			const embeddedFont = /font-family:"DejaVu Sans"[^}]*url\("data:font\/ttf;base64,([A-Za-z0-9+/=]+)"\)/.exec(styles)?.[1];
			const preview = document.createElement('img');
			preview.alt = 'Retained Arabic vector preview';
			preview.src = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
			document.body.append(preview);
			await preview.decode();
			const pdf = session.render_pdf_pages(new Uint32Array([0]), 'light');
			const download = document.createElement('a');
			download.textContent = 'Download retained Arabic PDF';
			download.download = 'retained-arabic.pdf';
			download.href = URL.createObjectURL(new Blob([pdf], { type: 'application/pdf' }));
			document.body.append(download);
			return {
				storedSource, source: [...parsed.querySelectorAll('text')].map(text => text.textContent).join(''),
				embeddedFont, images: parsed.querySelectorAll('image, foreignObject').length,
				previewWidth: preview.naturalWidth, previewHeight: preview.naturalHeight
			};
		} finally { session.free(); }
	}, { note: [...retainedPdfNote()], font: [...font] });
	expect(result.storedSource).toBe(retainedSource);
	expect(result.source).toBe(retainedSource);
	expect(result.images).toBe(0);
	expect([result.previewWidth, result.previewHeight]).toEqual([400, 400]);
	expect(result.embeddedFont).toBeDefined();
	expect(createHash('sha256').update(Buffer.from(result.embeddedFont!, 'base64')).digest('hex')).toBe(createHash('sha256').update(font).digest('hex'));
	const started = page.waitForEvent('download');
	await page.getByRole('link', { name: 'Download retained Arabic PDF', exact: true }).click();
	const download = await started;
	expect(download.suggestedFilename()).toBe('retained-arabic.pdf');
	const pdf = await PDFDocument.load(await readFile((await download.path())!));
	expect(pdf.getPageCount()).toBe(1);
	const objects = pdf.context.enumerateIndirectObjects().map(([, value]) => value);
	const dictionaries = objects.map(value => value instanceof PDFRawStream ? value.dict.toString() : value.toString()).join('\n');
	const contents = objects.filter((value): value is PDFRawStream => value instanceof PDFRawStream)
		.map(stream => Buffer.from(decodePDFRawStream(stream).decode()).toString('latin1')).join('\n');
	expect(dictionaries).toContain('/FontFile2');
	expect(dictionaries).toContain('/ToUnicode');
	expect(dictionaries).toContain('/StructTreeRoot');
	expect(dictionaries).not.toMatch(/\/Subtype \/Image\b/);
	expect(contents).toMatch(/\b(?:Tj|TJ)\b/);
	const actualText = [...contents.matchAll(/\/ActualText\b/g)].map(match => {
		const value = PDFObjectParser.forBytes(Buffer.from(contents.slice(match.index! + match[0].length), 'latin1'), pdf.context).parseObject();
		if (!(value instanceof PDFHexString || value instanceof PDFString)) throw new Error('PDF ActualText is not a string.');
		return { source: value.decodeText(), bytes: Buffer.from(value.asBytes()).toString('hex').toUpperCase() };
	}).filter(value => value.source === retainedSource);
	expect(actualText).toEqual([{ source: retainedSource, bytes: 'FEFF2066064406272069005F003100320033' }]);
	expect(fontRequests).toEqual([]);
	await expect(page.locator('canvas')).toHaveCount(0);
	await page.evaluate(() => {
		for (const node of document.querySelectorAll<HTMLImageElement | HTMLAnchorElement>('img[alt="Retained Arabic vector preview"], a[download="retained-arabic.pdf"]')) {
			URL.revokeObjectURL(node instanceof HTMLImageElement ? node.src : node.href);
			node.remove();
		}
	});
});
