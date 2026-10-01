import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { zipSync } from 'fflate';
import { PDFDocument, PDFRawStream } from 'pdf-lib';
import { expect, test } from '../fixtures/browser';
import { join, zero, u16, u32, f32, f64, frame } from '../fixtures/wdoc';

function collectionNote(): Buffer {
	const source = 'iiiWWW';
	const family = Buffer.from('DejaVu Sans', 'utf8');
	const name = join(zero(8), u16(family.length + 1), family, zero(1));
	const spans = [
		join(u16(20), ...[3, 0, source.length, 1].map(u32), f32(45)),
		join(u16(20), ...[1, 0, source.length, 1, 0xff000000].map(u32)),
		join(u16(16 + name.length), ...[4, 0, source.length, 1].map(u32), name)
	];
	const common = join(u32(source.length), Buffer.from(source, 'utf16le'), u32(spans.length), ...spans, u32(0), zero(16), zero(11));
	const base = frame(0, 1, join(u32(5500), u16(2), Buffer.from('tx'), zero(8), ...[10, 20, 310, 220].map(f64), zero(5)), f32(0));
	const payload = join(base, frame(6, 0), frame(7, 1, zero(0), join(u32(common.length), common)), frame(2, 0));
	const object = join(Buffer.from([2]), u16(0), u32(payload.length + 32), payload, zero(32));
	const header = join(zero(8), Buffer.from([1, 0, 5]), zero(5), ...[0, 400, 400, 0, 0].map(u32), u16(4), Buffer.from('face', 'utf16le'), zero(8), u32(5500), u32(4000));
	header.writeUInt32LE(header.length, 0);
	header.writeUInt32LE(header.length, 4);
	const layer = join(u32(20), zero(4), Buffer.from([2, 2, 0, 3, 0, 0, 0]), zero(5), u32(1), object, zero(32));
	return Buffer.from(zipSync({ 'face.page': join(header, u16(1), u16(0), layer, zero(32), Buffer.from('Page for SAMSUNG S-Pen SDK')) }));
}

test('standalone SVG paints the selected second face of a font collection', async ({ page, browserName }) => {
	test.skip(browserName !== 'chromium', 'Chromium preview and vector exports are the immediate target.');
	const [collection, selected, first] = await Promise.all([
		readFile(resolve('../crates/sdocx/tests/assets/fonts/Roboto-DejaVuSans.ttc')),
		readFile(resolve('../crates/sdocx/tests/assets/fonts/DejaVuSans.ttf')),
		readFile(resolve('../crates/sdocx/assets/fonts/Roboto-Regular.ttf'))
	]);
	expect(collection.subarray(0, 4).toString('ascii')).toBe('ttcf');
	expect(collection.readUInt32BE(8)).toBe(2);
	expect(createHash('sha256').update(collection).digest('hex')).toBe('05b90eb2586360bca28ff1852a12e0bda9933f344b0ae75b3465f22b73ed63ba');
	await page.route('https://rybbit.twango.dev/api/script.js', route => route.fulfill({ body: '' }));
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Your notes, in one place', exact: true })).toBeVisible();
	await page.evaluate(() => document.fonts.ready.then(() => undefined));
	const fontRequests: string[] = [];
	page.on('request', request => {
		if (/^https?:/.test(request.url()) && /\.(?:ttf|otf|woff2?)(?:\?|$)/.test(request.url())) fontRequests.push(request.url());
	});
	const result = await page.evaluate(async ({ note, collection, selected, first }) => {
		const module = await import(`${location.origin}/wasm/sdocx_wasm.js`);
		await module.default();
		const session = new module.DocumentSession(new Uint8Array(note));
		let svg: string;
		let pdf: number[];
		let invalidFont: string;
		let unchangedAfterInvalid: boolean;
		try {
			session.add_pdf_font(new Uint8Array(collection));
			svg = session.render_svg(0, 'light');
			try { session.add_pdf_font(new Uint8Array([1, 2, 3])); invalidFont = ''; }
			catch (error) { invalidFont = (error as Error).message; }
			unchangedAfterInvalid = session.render_svg(0, 'light') === svg;
			pdf = Array.from(session.render_pdf_pages(new Uint32Array([0]), 'light'));
		} finally { session.free(); }
		const parsed = new DOMParser().parseFromString(svg, 'image/svg+xml');
		const text = parsed.querySelector('text');
		if (!text) throw new Error('The collection fixture rendered no vector text.');
		const styles = [...parsed.querySelectorAll('style')];
		const isolated = parsed.documentElement;
		isolated.replaceChildren(...styles.map(style => style.cloneNode(true)), text.cloneNode(true));
		const scale = 4;
		isolated.setAttribute('viewBox', '0 0 320 140');
		isolated.setAttribute('width', String(320 * scale));
		isolated.setAttribute('height', String(140 * scale));
		const scan = (canvas: HTMLCanvasElement) => {
			const pixels = canvas.getContext('2d')!.getImageData(0, 0, canvas.width, canvas.height).data;
			let hash = 2166136261;
			let left = canvas.width;
			let right = -1;
			for (let offset = 0; offset < pixels.length; offset++) {
				hash = Math.imul(hash ^ pixels[offset], 16777619) >>> 0;
				if (offset % 4 === 3 && pixels[offset] > 0) {
					const x = Math.floor(offset / 4) % canvas.width;
					left = Math.min(left, x);
					right = Math.max(right, x);
				}
			}
			return { hash, inkWidth: (right - left + 1) / scale };
		};
		const raster = async (element: Element) => {
			const url = URL.createObjectURL(new Blob([new XMLSerializer().serializeToString(element)], { type: 'image/svg+xml' }));
			try {
				const image = new Image();
				image.src = url;
				await image.decode();
				const canvas = document.createElement('canvas');
				canvas.width = image.naturalWidth;
				canvas.height = image.naturalHeight;
				canvas.getContext('2d')!.drawImage(image, 0, 0);
				return scan(canvas);
			} finally { URL.revokeObjectURL(url); }
		};
		const referenceSvg = (font: string, family: string) => {
			const reference = isolated.cloneNode(true) as Element;
			for (const style of reference.querySelectorAll('style')) style.remove();
			const style = parsed.createElementNS('http://www.w3.org/2000/svg', 'style');
			style.textContent = `@font-face{font-family:"${family}";src:url("data:font/ttf;base64,${font}") format("truetype")}`;
			reference.prepend(style);
			for (const node of reference.querySelectorAll('[font-family]')) node.setAttribute('font-family', family);
			return reference;
		};
		const external = new FontFace('Sdocx Collection Reference', Uint8Array.from(atob(selected), character => character.charCodeAt(0)));
		await external.load();
		document.fonts.add(external);
		try {
			const canvas = document.createElement('canvas');
			canvas.width = 320 * scale;
			canvas.height = 140 * scale;
			const paint = canvas.getContext('2d')!;
			const glyphs = [...isolated.querySelectorAll<SVGTSpanElement>('tspan')].flatMap(span => {
				const characters = [...span.textContent!];
				const positions = span.getAttribute('x')!.trim().split(/\s+/).map(Number);
				if (positions.length !== characters.length) throw new Error('The collection fixture requires native scalar positions.');
				const y = Number(span.getAttribute('y'));
				paint.font = `${Number(span.getAttribute('font-size')) * scale}px "Sdocx Collection Reference"`;
				paint.fillStyle = span.getAttribute('fill')!;
				return characters.map((character, index) => {
					paint.fillText(character, positions[index] * scale, y * scale);
					return { character, x: positions[index] };
				});
			});
			return {
				source: text.textContent, glyphs, pdf, invalidFont, unchangedAfterInvalid, faceCount: styles.length,
				original: await raster(isolated), selected: await raster(referenceSvg(selected, 'SdocxSelectedFace')),
				first: await raster(referenceSvg(first, 'SdocxFirstFace')), expectedInkWidth: scan(canvas).inkWidth
			};
		} finally { document.fonts.delete(external); }
	}, { note: [...collectionNote()], collection: [...collection], selected: selected.toString('base64'), first: first.toString('base64') });
	expect(result.source).toBe('iiiWWW');
	expect(result.faceCount).toBe(1);
	expect(result.invalidFont).toContain('no usable PDF font');
	expect(result.unchangedAfterInvalid).toBe(true);
	for (const [index, x] of [10, 22.50244140625, 35.0048828125].entries()) {
		expect(result.glyphs[index].character).toBe('i');
		expect(result.glyphs[index].x).toBeCloseTo(x, 4);
	}
	expect(result.original.inkWidth).toBeGreaterThan(0);
	expect(result.original.hash).toBe(result.selected.hash);
	expect(result.original.hash).not.toBe(result.first.hash);
	expect(result.original.inkWidth).toBeCloseTo(result.expectedInkWidth, 0);
	const pdf = await PDFDocument.load(new Uint8Array(result.pdf));
	expect(pdf.getPageCount()).toBe(1);
	const dictionaries = pdf.context.enumerateIndirectObjects().map(([, value]) => value instanceof PDFRawStream ? value.dict.toString() : value.toString()).join('\n');
	expect(dictionaries).toContain('DejaVuSans');
	expect(dictionaries).toContain('/FontFile2');
	expect(dictionaries).toContain('/ToUnicode');
	expect(dictionaries).not.toMatch(/\/Subtype \/Image\b/);
	expect(fontRequests).toEqual([]);
	await expect(page.locator('canvas')).toHaveCount(0);
});
