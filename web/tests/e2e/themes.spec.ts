import { test, expect } from '../fixtures/browser';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { PDFDocument, PDFName, PDFRawStream, decodePDFRawStream } from 'pdf-lib';

interface ThemeRenderResult {
	pageIndex: number;
	mode: string;
	svg: string;
	defaultInk: string;
	changed: number;
	visible: number;
	background: string | null | undefined;
	images: (string | null)[];
	filters: number;
	paths: (string | null)[];
	pdf: number[];
}

async function rasterizeSvg(svg: string): Promise<Uint8ClampedArray> {
	const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
	try {
		const image = new Image();
		image.src = url;
		await image.decode();
		const canvas = document.createElement('canvas');
		canvas.width = 400;
		canvas.height = Math.round(400 * image.naturalHeight / image.naturalWidth);
		const context = canvas.getContext('2d')!;
		context.drawImage(image, 0, 0, canvas.width, canvas.height);
		return context.getImageData(0, 0, canvas.width, canvas.height).data;
	} finally {
		URL.revokeObjectURL(url);
	}
}

for (const fixture of ['01-basic-formatting', '02-shapes-and-dot-calibration', '03-image-placement', '04-marker4-highlighter']) {
	test(`${fixture} preserves visible vectors across document themes`, async ({ page }, testInfo) => {
		const path = resolve(`../hf/${fixture}.sdocx`);
		test.skip(!existsSync(path), 'External compatibility fixture is absent');
		await page.route('**/__theme.sdocx', route => route.fulfill({ body: readFileSync(path) }));
		await page.goto('/');
		const wasm = await test.step('Initialize browser WASM', () => page.evaluateHandle(async () => {
			const moduleUrl = '/wasm/sdocx_wasm.js';
			const module = await import(moduleUrl) as {
				default: () => Promise<unknown>;
				DocumentSession: new (bytes: Uint8Array) => {
					render_svg(page: number, color: string): string;
					inspection(): { layout: { pages: { source_page_index: number }[] } };
					render_pdf_pages(pages: Uint32Array, color: string): Uint8Array;
					debug(request: string): string;
					free(): void;
				};
			};
			await module.default();
			return module;
		}));
		const session = await test.step('Read and parse theme fixture', () => wasm.evaluateHandle(async (module) => {
			const bytes = new Uint8Array(await (await fetch('/__theme.sdocx')).arrayBuffer());
			return new module.DocumentSession(bytes);
		})).catch(async (error) => {
			await wasm.dispose();
			throw error;
		});
		const result: ThemeRenderResult[] = [];
		try {
			const pages = await test.step('Inspect physical pages', () => session.evaluate(session => session.inspection().layout.pages.map(page => page.source_page_index)));
			for (const [pageIndex, sourcePage] of pages.entries()) {
				for (const mode of ['auto', 'light', 'dark']) {
					const row = await test.step(`Page ${pageIndex + 1}, ${mode}`, async () => {
						const svg = await test.step('Render exported SVG', () => session.evaluate((session, { pageIndex, mode }) => session.render_svg(pageIndex, mode), { pageIndex, mode }));
						const replay = await test.step('Render replay SVG', () => session.evaluate((session, { sourcePage, mode }) =>
							JSON.parse(session.debug(JSON.stringify({ kind: 'replay-svg', page: sourcePage, colorMode: mode }))) as { svg: string; defaultInk: string },
							{ sourcePage, mode }));
						const normalPixels = await test.step('Decode and rasterize exported SVG', () => page.evaluateHandle(rasterizeSvg, svg));
						try {
							const replayPixels = await test.step('Decode and rasterize replay SVG', () => page.evaluateHandle(rasterizeSvg, replay.svg));
							try {
								const pixels = await test.step('Compare replay pixels and visible ink', () => normalPixels.evaluate((normal, { replay, mode }) => {
									let changed = 0, visible = 0;
									const paper = mode === 'dark' ? 37 : 252;
									for (let i = 0; i < normal.length; i += 4) {
										if ([0, 1, 2, 3].some(c => normal[i + c] !== replay[i + c])) changed++;
										if ([0, 1, 2].some(c => Math.abs(normal[i + c] - paper) > 40)) visible++;
									}
									return { changed, visible };
								}, { replay: replayPixels, mode }));
								const vectors = await test.step('Inspect SVG vectors and assets', () => page.evaluate(svg => {
									const xml = new DOMParser().parseFromString(svg, 'image/svg+xml');
									return {
										background: xml.querySelector('rect')?.getAttribute('fill'),
										images: [...xml.querySelectorAll('image')].map(image => image.getAttribute('href')),
										filters: xml.querySelectorAll('filter').length,
										paths: [...xml.querySelectorAll('path')].map(path => path.getAttribute('d'))
									};
								}, svg));
								const pdf = await test.step('Render vector PDF', () => session.evaluate((session, { pageIndex, mode }) =>
									Array.from(session.render_pdf_pages(new Uint32Array([pageIndex]), mode)), { pageIndex, mode }));
								return { pageIndex, mode, svg, defaultInk: replay.defaultInk, ...pixels, ...vectors, pdf };
							} finally { await replayPixels.dispose(); }
						} finally { await normalPixels.dispose(); }
					});
					result.push(row);
				}
			}
		} finally {
			try { await session.evaluate(session => session.free()); }
			finally { await session.dispose(); await wasm.dispose(); }
		}
		for (const row of result) {
			await test.step(`Verify page ${row.pageIndex + 1}, ${row.mode}`, async () => {
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
			});
		}
	});
}
