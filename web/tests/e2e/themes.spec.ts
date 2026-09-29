import { test, expect } from '../fixtures/browser';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { PDFDocument, PDFName, PDFRawStream, decodePDFRawStream } from 'pdf-lib';

for (const fixture of ['01-basic-formatting', '02-shapes-and-dot-calibration', '03-image-placement', '04-marker4-highlighter']) {
	test(`${fixture} preserves visible vectors across document themes`, async ({ page }, testInfo) => {
		const path = resolve(`../hf/${fixture}.sdocx`);
		test.skip(!existsSync(path), 'External compatibility fixture is absent');
		await page.route('**/__theme.sdocx', route => route.fulfill({ body: readFileSync(path) }));
		await page.goto('/');
		const result = await page.evaluate(async () => {
			const moduleUrl = '/wasm/sdocx_wasm.js';
			const wasm = await import(moduleUrl) as {
				default: () => Promise<unknown>;
				DocumentSession: new (bytes: Uint8Array) => {
					render_svg(page: number, color: string): string;
					inspection(): { layout: { pages: { source_page_index: number }[] } };
					render_pdf_pages(pages: Uint32Array, color: string): Uint8Array;
					add_pdf_font(bytes: Uint8Array): void;
					debug(request: string): string;
					free(): void;
				};
			};
			await wasm.default();
			const session = new wasm.DocumentSession(new Uint8Array(await (await fetch('/__theme.sdocx')).arrayBuffer()));
			const pixels = async (svg: string) => {
				const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
				try {
					const image = new Image(); image.src = url; await image.decode();
					const canvas = document.createElement('canvas'); canvas.width = 400;
					canvas.height = Math.round(400 * image.naturalHeight / image.naturalWidth);
					const context = canvas.getContext('2d')!; context.drawImage(image, 0, 0, canvas.width, canvas.height);
					return context.getImageData(0, 0, canvas.width, canvas.height).data;
				} finally { URL.revokeObjectURL(url); }
			};
			try {
				for (const name of ['Roboto-Regular.ttf', 'Roboto-Bold.ttf', 'Roboto-Italic.ttf', 'Roboto-BoldItalic.ttf']) {
					session.add_pdf_font(new Uint8Array(await (await fetch(`/pdf-fonts/${name}`)).arrayBuffer()));
				}
				const rows = [];
				for (const [pageIndex, layoutPage] of session.inspection().layout.pages.entries()) {
					for (const mode of ['auto', 'light', 'dark']) {
						const svg = session.render_svg(pageIndex, mode);
						const replay = JSON.parse(session.debug(JSON.stringify({ kind: 'replay-svg', page: layoutPage.source_page_index, colorMode: mode }))) as { svg: string; defaultInk: string };
						const xml = new DOMParser().parseFromString(svg, 'image/svg+xml');
						const normalPixels = await pixels(svg), replayPixels = await pixels(replay.svg);
						let changed = 0, visible = 0;
						const paper = mode === 'dark' ? 37 : 252;
						for (let i = 0; i < normalPixels.length; i += 4) {
							if ([0, 1, 2, 3].some(c => normalPixels[i + c] !== replayPixels[i + c])) changed++;
							if ([0, 1, 2].some(c => Math.abs(normalPixels[i + c] - paper) > 40)) visible++;
						}
						rows.push({ pageIndex, mode, svg, changed, visible, defaultInk: replay.defaultInk,
							background: xml.querySelector('rect')?.getAttribute('fill'),
							images: [...xml.querySelectorAll('image')].map(image => image.getAttribute('href')),
							filters: xml.querySelectorAll('filter').length,
							paths: [...xml.querySelectorAll('path')].map(p => p.getAttribute('d')),
							pdf: Array.from(session.render_pdf_pages(new Uint32Array([pageIndex]), mode)) });
					}
				}
				return rows;
			} finally { session.free(); }
		});
		for (const row of result) {
			const original = result.find(r => r.pageIndex === row.pageIndex && r.mode === 'auto')!;
			expect(row.background).toBe(row.mode === 'dark' ? '#252525' : '#fcfcfc');
			expect(row.defaultInk).toBe(row.mode === 'dark' ? '#ffffff' : '#1a1a1a');
			expect(row.changed, 'Replay and exported SVG pixels').toBe(0);
			expect(row.visible, 'Content distinguishable from paper').toBeGreaterThan(0);
			expect(row.filters).toBe(0);
			expect(row.images).toEqual(original.images);
			if (fixture === '03-image-placement') expect(row.images.length).toBeGreaterThan(0);
			else expect(row.images).toHaveLength(0);
			expect(row.paths, 'Theme must not change vector geometry').toEqual(original.paths);
			const pdf = await PDFDocument.load(new Uint8Array(row.pdf));
			const objects = pdf.context.enumerateIndirectObjects().map(([, value]) => value);
			const dictionaries = objects.map(value => value instanceof PDFRawStream ? value.dict.toString() : value.toString()).join('\n');
			if (fixture === '03-image-placement') expect(dictionaries).toContain('/Subtype /Image');
			else expect(dictionaries).not.toContain('/Subtype /Image');
			const contents = objects.filter((v): v is PDFRawStream => v instanceof PDFRawStream && v.dict.get(PDFName.of('Subtype'))?.toString() !== '/Image')
				.map(v => Buffer.from(decodePDFRawStream(v).decode()).toString('latin1')).join('\n');
			expect(contents).toMatch(row.mode === 'dark' ? /0\.145\d+ g\b/ : /0\.988\d+ g\b/);
			if (fixture === '04-marker4-highlighter') expect(dictionaries).toContain(row.mode === 'dark' ? '/Lighten' : '/Darken');
			await testInfo.attach(`page-${row.pageIndex + 1}-${row.mode}.svg`, { body: row.svg, contentType: 'image/svg+xml' });
		}
	});
}
