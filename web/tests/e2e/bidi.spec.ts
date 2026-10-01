import { expect, test } from '../fixtures/browser';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { zipSync } from 'fflate';
import { PDFDocument, PDFRawStream, decodePDFRawStream } from 'pdf-lib';

interface BidiCase {
	source: string;
	width: number;
	characters: string[];
	x: number[];
	line: number[];
}

// Independent pinned positions from crates/sdocx/tests/vector_bidi_text.rs.
const cases: BidiCase[] = [
	{ source: 'A\u202eBC\u202cD', width: 500, characters: ['A', 'B', 'C', 'D'], x: [0, 58.64501953125, 29.35546875, 86.66015625], line: [0, 0, 0, 0] },
	{ source: 'A\u202eAV\u202cD', width: 500, characters: ['A', 'A', 'V', 'D'], x: [0, 56.337890625, 29.35546875, 85.693359375], line: [0, 0, 0, 0] },
	{ source: 'A\u202dBC\u202cD', width: 500, characters: ['A', 'B', 'C', 'D'], x: [0, 29.35546875, 57.37060546875, 86.66015625], line: [0, 0, 0, 0] },
	{ source: 'A\u202eB C\u202cD', width: 85, characters: ['A', 'B', ' ', 'C', 'D'], x: [0, 40.49560546875, 29.35546875, 0, 29.28955078125], line: [0, 0, 0, 1, 1] }
];

const join = (...parts: Uint8Array[]) => Buffer.concat(parts);
const zero = (length: number) => Buffer.alloc(length);
const u16 = (value: number) => { const bytes = Buffer.alloc(2); bytes.writeUInt16LE(value); return bytes; };
const u32 = (value: number) => { const bytes = Buffer.alloc(4); bytes.writeUInt32LE(value); return bytes; };
const f32 = (value: number) => { const bytes = Buffer.alloc(4); bytes.writeFloatLE(value); return bytes; };
const f64 = (value: number) => { const bytes = Buffer.alloc(8); bytes.writeDoubleLE(value); return bytes; };

function frame(kind: number, fields: number, fixed = zero(0), flexible = zero(0)) {
	const offset = 18 + fixed.length;
	return join(u32(offset + flexible.length), u16(kind), u32(offset), Buffer.from([2]), u16(kind === 0 ? 8 : 0), Buffer.from([4]), u32(fields), fixed, flexible);
}

function bidiNote(example: BidiCase): Buffer {
	const label = Buffer.from(example.source, 'utf16le');
	const fontSize = join(u16(20), ...[3, 0, label.length / 2, 1].map(u32), f32(45));
	const color = join(u16(20), ...[1, 0, label.length / 2, 1, 0xff000000].map(u32));
	const common = join(u32(label.length / 2), label, u32(2), fontSize, color, u32(0), zero(16), zero(11));
	const base = frame(0, 1, join(u32(5500), u16(2), Buffer.from('tx'), zero(8), ...[10, 20, 10 + example.width, 620].map(f64), zero(5)), f32(0));
	const payload = join(base, frame(6, 0), frame(7, 1, zero(0), join(u32(common.length), common)), frame(2, 0));
	const object = join(Buffer.from([2]), u16(0), u32(payload.length + 32), payload, zero(32));
	const header = join(zero(8), Buffer.from([1, 0, 5]), zero(5), ...[0, 700, 800, 0, 0].map(u32), u16(4), Buffer.from('bidi', 'utf16le'), zero(8), u32(5500), u32(4000));
	header.writeUInt32LE(header.length, 0);
	header.writeUInt32LE(header.length, 4);
	const layer = join(u32(20), zero(4), Buffer.from([2, 2, 0, 3, 0, 0, 0]), zero(5), u32(1), object, zero(32));
	return Buffer.from(zipSync({ 'bidi.page': join(header, u16(1), u16(0), layer, zero(32), Buffer.from('Page for SAMSUNG S-Pen SDK')) }));
}

test('native bidi SVG images retain logical source, glyph positions and vector PDF text', async ({ page, browserName }) => {
	test.skip(browserName !== 'chromium', 'Chromium preview and vector exports are the immediate target.');
	await page.route('https://rybbit.twango.dev/api/script.js', route => route.fulfill({ body: '' }));
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Your notes, in one place', exact: true })).toBeVisible();
	await page.evaluate(() => document.fonts.ready.then(() => undefined));
	const fontRequests: string[] = [];
	page.on('request', request => {
		if (/^https?:/.test(request.url()) && /\.(?:ttf|otf|woff2?)(?:\?|$)/.test(request.url())) fontRequests.push(request.url());
	});
	const font = await readFile(resolve('../crates/sdocx/assets/fonts/Roboto-Regular.ttf'));
	const results = await page.evaluate(async ({ examples, font }) => {
		const module = await import(`${location.origin}/wasm/sdocx_wasm.js`);
		await module.default();
		const reference = new FontFace('Sdocx Bidi Reference Roboto', new Uint8Array(font));
		await reference.load();
		document.fonts.add(reference);
		const scale = 4;
		const width = 700;
		const height = 220;
		const formatting = (character: string) => /^[\u202a-\u202e\u2066-\u2069]$/.test(character);
		const scan = (canvas: HTMLCanvasElement) => {
			const pixels = canvas.getContext('2d')!.getImageData(0, 0, canvas.width, canvas.height).data;
			let left = canvas.width;
			let right = -1;
			let top = canvas.height;
			let bottom = -1;
			let hash = 2166136261;
			for (let offset = 0; offset < pixels.length; offset++) {
				hash = Math.imul(hash ^ pixels[offset], 16777619) >>> 0;
				if (offset % 4 === 3 && pixels[offset] > 0) {
					const x = Math.floor(offset / 4) % canvas.width;
					const y = Math.floor(Math.floor(offset / 4) / canvas.width);
					left = Math.min(left, x);
					right = Math.max(right, x);
					top = Math.min(top, y);
					bottom = Math.max(bottom, y);
				}
			}
			return { bounds: [left, top, right, bottom].map(value => value / scale), hash };
		};
		const raster = async (svg: Element) => {
			const url = URL.createObjectURL(new Blob([new XMLSerializer().serializeToString(svg)], { type: 'image/svg+xml' }));
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
		try {
			const output = [];
			for (const example of examples) {
				const session = new module.DocumentSession(new Uint8Array(example.note));
				let svg: string;
				let pdf: number[];
				try {
					svg = session.render_svg(0, 'light');
					pdf = Array.from(session.render_pdf(0, 'light'));
				} finally { session.free(); }
				const document = new DOMParser().parseFromString(svg, 'image/svg+xml');
				const texts = [...document.querySelectorAll('text')];
				const source = texts.map(text => text.textContent).join('');
				const glyphs = [...document.querySelectorAll<SVGTSpanElement>('tspan')].flatMap(span => {
					const characters = [...span.textContent!];
					if (characters.every(formatting)) return [];
					const positions = span.getAttribute('x')?.trim().split(/\s+/).map(Number);
					if (!positions || positions.length !== characters.length) throw new Error('Native bidi characters require explicit scalar positions.');
					return characters.flatMap((character, index) => formatting(character) ? [] : [{ character, x: positions[index], y: Number(span.getAttribute('y')), size: Number(span.getAttribute('font-size')) }]);
				});
				if (!glyphs.length) throw new Error('The bidi fixture produced no positioned glyphs.');
				const baseline = glyphs[0].y;
				const styles = [...document.querySelectorAll('style')];
				const isolated = document.documentElement;
				isolated.replaceChildren(...styles.map(style => style.cloneNode(true)), ...texts.map(text => text.cloneNode(true)));
				isolated.setAttribute('viewBox', `0 0 ${width} ${height}`);
				isolated.setAttribute('width', String(width * scale));
				isolated.setAttribute('height', String(height * scale));
				const original = await raster(isolated);
				const referenceCanvas = globalThis.document.createElement('canvas');
				referenceCanvas.width = width * scale;
				referenceCanvas.height = height * scale;
				const paint = referenceCanvas.getContext('2d')!;
				paint.font = `${45 * scale}px "Sdocx Bidi Reference Roboto"`;
				paint.fillStyle = '#000000';
				example.characters.forEach((character, index) => paint.fillText(character, (10 + example.x[index]) * scale, (baseline + 60.75 * example.line[index]) * scale));
				const expected = scan(referenceCanvas);
				const canonical = isolated.cloneNode(false) as Element;
				canonical.append(...styles.map(style => style.cloneNode(true)));
				example.characters.forEach((character, index) => {
					const glyph = document.createElementNS('http://www.w3.org/2000/svg', 'text');
					glyph.setAttribute('x', (10 + example.x[index]).toFixed(5));
					glyph.setAttribute('y', (baseline + 60.75 * example.line[index]).toFixed(5));
					glyph.setAttribute('font-family', 'Roboto');
					glyph.setAttribute('font-size', '45');
					glyph.setAttribute('fill', '#000000');
					glyph.textContent = character;
					canonical.append(glyph);
				});
				const expectedPixels = await raster(canonical);
				const probe = isolated.cloneNode(true) as Element;
				for (const style of probe.querySelectorAll('style')) {
					style.textContent = style.textContent!.replace('font-family:"Roboto"', 'font-family:"SdocxBidiEmbeddedProbe"');
				}
				for (const node of probe.querySelectorAll('[font-family]')) {
					node.setAttribute('font-family', node.getAttribute('font-family')!.replace(/^("?)Roboto\1(?=,|$)/, '"SdocxBidiEmbeddedProbe"'));
				}
				const renamed = await raster(probe);
				for (const style of probe.querySelectorAll('style')) style.remove();
				for (const node of probe.querySelectorAll('[font-family]')) node.setAttribute('font-family', 'SdocxBidiEmbeddedProbe, monospace');
				const fallback = await raster(probe);
				output.push({ source, glyphs, original, expected, expectedPixels, renamed, fallback, embedded: styles.length === 1 && styles[0].textContent!.includes('data:font/ttf;base64,'), pdf });
			}
			return output;
		} finally { document.fonts.delete(reference); }
	}, { examples: cases.map(example => ({ ...example, note: [...bidiNote(example)] })), font: [...font] });
	for (const [index, result] of results.entries()) {
		const example = cases[index];
		expect(result.source).toBe(example.source);
		expect(result.glyphs.map(glyph => glyph.character)).toEqual(example.characters);
		for (const [glyphIndex, glyph] of result.glyphs.entries()) {
			expect(glyph.x).toBeCloseTo(10 + example.x[glyphIndex], 4);
			expect(glyph.y - result.glyphs[0].y).toBeCloseTo(60.75 * example.line[glyphIndex], 4);
			expect(glyph.size).toBe(45);
		}
		expect(result.embedded).toBe(true);
		expect(result.original.hash).toBe(result.expectedPixels.hash);
		expect(result.original.hash).toBe(result.renamed.hash);
		expect(result.original.hash).not.toBe(result.fallback.hash);
		for (const [bound, value] of result.original.bounds.entries()) expect(value).toBeCloseTo(result.expected.bounds[bound], 0);
		const pdf = await PDFDocument.load(new Uint8Array(result.pdf));
		expect(pdf.getPageCount()).toBe(1);
		const objects = pdf.context.enumerateIndirectObjects().map(([, value]) => value);
		const dictionaries = objects.map(value => value instanceof PDFRawStream ? value.dict.toString() : value.toString()).join('\n');
		const contents = objects.filter((value): value is PDFRawStream => value instanceof PDFRawStream)
			.map(stream => Buffer.from(decodePDFRawStream(stream).decode()).toString('latin1')).join('\n');
		expect(dictionaries).toContain('/FontFile2');
		expect(dictionaries).toContain('/ToUnicode');
		expect(dictionaries).not.toContain('/Subtype /Image');
		expect(contents).toContain('/ActualText');
		expect(contents).toMatch(/\b(?:Tj|TJ)\b/);
	}
	expect(fontRequests).toEqual([]);
	await expect(page.locator('canvas')).toHaveCount(0);
});
