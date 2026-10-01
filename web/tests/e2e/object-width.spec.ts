import { createHash } from 'node:crypto';
import { zipSync } from 'fflate';
import { PDFDocument, PDFRawStream, decodePDFRawStream } from 'pdf-lib';
import { expect, test } from '../fixtures/browser';
import { join, zero, u16, u32, f32, f64, frame } from '../fixtures/wdoc';

interface WidthCase {
	width: number;
	constraint: number;
	bX: number;
	wrapped: boolean;
}

// Independent native expectations in crates/sdocx/tests/object_width_feedback.rs.
const cases: WidthCase[] = [
	{ width: 260, constraint: 0, bX: 35.5234375, wrapped: false },
	{ width: 260, constraint: 1, bX: 67.5234375, wrapped: false },
	{ width: 260, constraint: 2, bX: 67.5234375, wrapped: false },
	{ width: 70, constraint: 1, bX: 0, wrapped: true },
	{ width: 70, constraint: 2, bX: 0, wrapped: true }
];

const utf16 = (value: string) => join(u16(value.length), Buffer.from(value, 'utf16le'));
const record = (fixed: Buffer) => join(u32(0), Buffer.from([1, 0, 1, 0]), fixed);

function base(bbox: number[]) {
	return frame(0, 1, join(u32(5500), u16(2), Buffer.from('tx'), zero(8), ...bbox.map(f64), zero(5)), f32(0));
}

function text(source: string, objectSpans = zero(8), cell = false) {
	const label = Buffer.from(source, 'utf16le');
	const spans = source ? [
		join(u16(20), ...[3, 0, source.length, 1].map(u32), f32(10)),
		join(u16(20), ...[1, 0, source.length, 1, 0xff000000].map(u32))
	] : [];
	const paragraphs = cell ? join(u32(1), u16(20), ...[4, 0, 0, 1].map(u32), f32(1)) : u32(0);
	const sections = source ? join(u16(1), u32(0), u32(source.length)) : u16(0);
	const common = join(u32(source.length), label, u32(spans.length), ...spans, paragraphs, zero(16), zero(1), sections, objectSpans);
	return join(base([0, 0, 0, 0]), frame(6, 0), frame(7, 1, zero(0), join(u32(common.length), common)));
}

function table() {
	const content = text('T', zero(8), true);
	const cell = record(join(...[0, 1, 1, 0].map(u32), ...[0, 0, 20, 20].map(f64), zero(1), u32(content.length), content));
	const row = record(join(f32(20), u32(0), u32(1), u32(cell.length), cell));
	return join(base([0, 0, 20, 20]), frame(22, 12, zero(0), join(u32(1), f32(60), u32(1), u32(row.length), row), 4));
}

function flowNote(example: WidthCase) {
	const object = table();
	const objectSpans = join(u32(1), u32(0), u32(1), u32(object.length + 20), u32(object.length), u32(22), object, u32(1), u32(1), u32(example.constraint));
	const title = text('');
	const body = text('A\ufffcB', objectSpans);
	const fixedNote = join(zero(4), Buffer.from([1, 0, 1, 0]), u32(5500), utf16('width'), u32(12), zero(16), ...[example.width, 500, 0, 0, 4000].map(u32), u32(title.length), title, u32(body.length), body, u32(360), u32(500));
	fixedNote.writeUInt32LE(fixedNote.length, 0);
	const note = join(fixedNote, utf16('Samsung Notes'));
	const header = join(zero(8), Buffer.from([1, 0, 5]), zero(5), ...[0, example.width, 500, 0, 0].map(u32), utf16('width'), zero(8), u32(5500), u32(4000));
	header.writeUInt32LE(header.length, 0);
	header.writeUInt32LE(header.length, 4);
	const layer = join(u32(20), zero(4), Buffer.from([2, 2, 0, 3, 0, 0, 0]), zero(5), u32(0), zero(32));
	const tag = join(u32(5500), utf16('width'), zero(8), u32(0), utf16(''), u32(example.width), f32(500), utf16('Samsung Notes'), u32(4), u32(4), utf16(''), u32(4000), zero(8), u32(0), u16(0), Buffer.from('Document for S-Pen SDK'));
	return zipSync({
		'note.note': join(note, createHash('sha256').update(note).digest()),
		'width.page': join(header, u16(1), u16(0), layer, zero(32), Buffer.from('Page for SAMSUNG S-Pen SDK')),
		'end_tag.bin': join(u16(tag.length), tag)
	});
}

test('native Flow uses prepared table width for following text and wrapping', async ({ page, browserName }) => {
	test.skip(browserName !== 'chromium', 'Chromium preview and vector exports are the immediate target.');
	await page.route('https://rybbit.twango.dev/api/script.js', route => route.fulfill({ body: '' }));
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Your notes, in one place', exact: true })).toBeVisible();
	const results = await page.evaluate(async notes => {
		const module = await import(`${location.origin}/wasm/sdocx_wasm.js`);
		await module.default();
		return notes.map(note => {
			const session = new module.DocumentSession(new Uint8Array(note));
			try {
				const inspection = session.inspection().document;
				const body = inspection.metadata.note_text;
				const storedTable = body.object_spans[0].content.Table;
				const svg = session.render_svg(0, 'light');
				const parsed = new DOMParser().parseFromString(svg, 'image/svg+xml');
				const attached = document.importNode(parsed.documentElement, true) as unknown as SVGSVGElement;
				attached.style.position = 'absolute';
				attached.style.opacity = '0';
				document.body.append(attached);
				try {
					const point = (node: SVGGraphicsElement, x: number, y: number) => {
						const transform = attached.getCTM()!.inverse().multiply(node.getCTM()!);
						const position = new DOMPoint(x, y).matrixTransform(transform);
						return { x: position.x, y: position.y };
					};
					const glyph = (source: string) => {
						const spans = [...attached.querySelectorAll<SVGTSpanElement>('tspan')].filter(span => span.textContent === source);
						if (spans.length !== 1) throw new Error(`Expected one native glyph ${source}, got ${spans.length}.`);
						return point(spans[0], Number(spans[0].getAttribute('x')), Number(spans[0].getAttribute('y')));
					};
					const rectangle = attached.querySelector<SVGRectElement>('[data-sdocx-object="table"] rect[fill]:not([fill="none"])');
					if (!rectangle) throw new Error('The native table did not produce a vector rectangle.');
					return {
						storedSource: body.text, storedWidth: storedTable.bbox.x_max - storedTable.bbox.x_min,
						columnWidths: storedTable.column_widths, a: glyph('A'), b: glyph('B'), t: glyph('T'),
						table: { ...point(rectangle, Number(rectangle.getAttribute('x')), Number(rectangle.getAttribute('y'))), width: Number(rectangle.getAttribute('width')) },
						text: [...attached.querySelectorAll('text')].map(node => node.textContent).join(''),
						imageCount: attached.querySelectorAll('image, foreignObject').length,
						pdf: Array.from(session.render_pdf(0, 'light')) as number[]
					};
				} finally { attached.remove(); }
			} finally { session.free(); }
		});
	}, cases.map(example => [...flowNote(example)]));
	for (const [index, result] of results.entries()) {
		const example = cases[index];
		expect(result.storedSource).toBe('A\ufffcB');
		expect(result.storedWidth).toBe(20);
		expect(result.columnWidths).toEqual([60]);
		expect(result.a.x).toBe(0);
		expect(result.b.x, `width ${example.width}, constraint ${example.constraint}`).toBeCloseTo(example.bX, 4);
		if (example.wrapped) expect(result.b.y).toBeGreaterThan(result.a.y);
		else expect(result.b.y).toBe(result.a.y);
		expect(result.table.x).toBe(example.constraint === 0 ? 10 : 11);
		expect(result.table.width).toBe(61);
		expect(result.text).toBe(example.wrapped ? 'ATB' : 'ABT');
		expect(result.imageCount).toBe(0);
		const pdf = await PDFDocument.load(new Uint8Array(result.pdf));
		expect(pdf.getPageCount()).toBe(1);
		const objects = pdf.context.enumerateIndirectObjects().map(([, value]) => value);
		const dictionaries = objects.map(value => value instanceof PDFRawStream ? value.dict.toString() : value.toString()).join('\n');
		const contents = objects.filter((value): value is PDFRawStream => value instanceof PDFRawStream)
			.map(stream => Buffer.from(decodePDFRawStream(stream).decode()).toString('latin1')).join('\n');
		expect(dictionaries).toContain('/FontFile2');
		expect(dictionaries).toContain('/ToUnicode');
		expect(dictionaries).not.toMatch(/\/Subtype \/Image\b/);
		expect(contents).toMatch(/\b(?:Tj|TJ)\b/);
	}
	await expect(page.locator('canvas')).toHaveCount(0);
});
