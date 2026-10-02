import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { zipSync } from 'fflate';
import { PDFDocument, PDFHexString, PDFObjectParser, PDFRawStream, PDFString, decodePDFRawStream } from 'pdf-lib';
import { expect, test } from '../fixtures/browser';
import { join, zero, u16, u32, f32, f64, frame } from '../fixtures/wdoc';

interface CapturedRun {
	range_inclusive: [number, number];
	position_bits: number[];
	origin_bits: [number, number];
}

interface CapturedCell {
	name: string;
	text_utf8: string;
	requested_font_size_bits: number | null;
	measure_widths: number[];
	emitted_runs: { runs: CapturedRun[] };
}

const float = (bits: number) => {
	const bytes = Buffer.alloc(4);
	bytes.writeUInt32LE(bits);
	return bytes.readFloatLE();
};

function cellNote(source: CapturedCell, offset: number) {
	const width = source.measure_widths[0];
	const bbox = [0, 0, width, 1000];
	const base = () => frame(0, 1, join(u32(5500), u16(2), Buffer.from('tx'), zero(8), ...bbox.map(f64), zero(5)), f32(0));
	const label = Buffer.from(source.text_utf8, 'utf16le');
	const spans = source.requested_font_size_bits === null ? [] : [
		join(u16(20), ...[3, 0, source.text_utf8.length, 1].map(u32), u32(source.requested_font_size_bits)),
		join(u16(20), ...[1, 0, source.text_utf8.length, 1, 0xff252525].map(u32))
	];
	const paragraph = join(u32(1), u16(16), ...[3, 0, 1, 2].map(u32));
	const common = join(u32(source.text_utf8.length), label, u32(spans.length), ...spans, paragraph, zero(16), zero(11));
	const text = join(base(), frame(6, 0), frame(7, 1, zero(0), join(u32(common.length), common)));
	const record = (fixed: Buffer) => join(u32(0), Buffer.from([1, 0, 1, 0]), fixed);
	const cell = record(join(...[0, 1, 1, 0].map(u32), ...bbox.map(f64), zero(1), u32(text.length), text));
	const row = record(join(f32(1000), u32(0), u32(1), u32(cell.length), cell));
	const table = join(base(), frame(22, 12, zero(0), join(u32(1), f32(width), u32(1), u32(row.length), row), 4));
	const objectSpan = join(u32(1), u32(0), u32(1), u32(table.length + 20), u32(table.length), u32(22), table, u32(0), u32(1), u32(0));
	const bodyCommon = join(u32(1), u16(0xfffc), u32(0), u32(0), ...[offset, 0, 0, 0].map(f32), zero(1), u16(1), u32(0), u32(1), objectSpan);
	const body = join(base(), frame(6, 0), frame(7, 1, zero(0), join(u32(bodyCommon.length), bodyCommon)));
	const emptyCommon = join(u32(0), u32(0), u32(0), zero(16), zero(11));
	const title = join(base(), frame(6, 0), frame(7, 1, zero(0), join(u32(emptyCommon.length), emptyCommon)));
	const utf16 = (value: string) => join(u16(value.length), Buffer.from(value, 'utf16le'));
	const note = join(zero(4), Buffer.from([1, 0, 1, 0]), u32(5500), utf16('cell'), u32(12), zero(16), ...[800, 1600, 0, 0, 4000].map(u32), u32(title.length), title, u32(body.length), body, u32(360), u32(1600));
	note.writeUInt32LE(note.length, 0);
	const completeNote = join(note, utf16('Samsung Notes'));
	const header = join(zero(8), Buffer.from([1, 0, 5]), zero(5), ...[0, 800, 1600, 0, 0].map(u32), u16(4), Buffer.from('cell', 'utf16le'), zero(8), u32(5500), u32(4000));
	header.writeUInt32LE(header.length, 0);
	header.writeUInt32LE(header.length, 4);
	const layer = join(u32(20), zero(4), Buffer.from([2, 2, 0, 3, 0, 0, 0]), zero(5), u32(0), zero(32));
	const tag = join(u32(5500), utf16('cell'), zero(8), u32(0), utf16(''), u32(800), f32(1600), utf16('Samsung Notes'), u32(4), u32(4), utf16(''), u32(4000), zero(8), u32(0), u16(0), Buffer.from('Document for S-Pen SDK'));
	return zipSync({
		'note.note': join(completeNote, createHash('sha256').update(completeNote).digest()),
		'cell.page': join(header, u16(1), u16(0), layer, zero(32), Buffer.from('Page for SAMSUNG S-Pen SDK')),
		'end_tag.bin': join(u16(tag.length), tag)
	});
}

test('certified cell text preserves captured runs, wrapping and translated selectable vector exports', async ({ page, browserName }) => {
	test.skip(browserName !== 'chromium', 'Chromium preview and vector exports are the immediate target.');
	const bytes = await readFile(resolve('../conformance/table-text-cell-emission.json'));
	expect(createHash('sha256').update(bytes).digest('hex')).toBe('0098cfcd93274b210892fd0653677fd4cef3a35ebc4f0419b4b661857cbfa4ce');
	const capture = JSON.parse(bytes.toString()) as { cases: CapturedCell[] };
	const cases = ['default-narrow', 'fractional-width', 'consecutive-newlines'].map(name => capture.cases.find(value => value.name === name)!);
	await page.route('https://rybbit.twango.dev/api/script.js', route => route.fulfill({ body: '' }));
	await page.goto('/');
	const results = await page.evaluate(async notes => {
		const module = await import(`${location.origin}/wasm/sdocx_wasm.js`);
		await module.default();
		return notes.map(note => {
			const session = new module.DocumentSession(new Uint8Array(note));
			try {
				const svg = session.render_svg(0, 'light');
				const document = new DOMParser().parseFromString(svg, 'image/svg+xml');
				return {
					constraint: session.inspection().document.metadata.note_text.object_spans[0].layout_constraint,
					groups: [...document.querySelectorAll('text')].map(text => ({
						source: text.textContent,
						spans: [...text.querySelectorAll('tspan')].map(span => ({
							x: span.getAttribute('x')!.trim().split(/\s+/).map(Number),
							y: Number(span.getAttribute('y')),
							size: Number(span.getAttribute('font-size'))
						}))
					})),
					images: document.querySelectorAll('image, foreignObject').length,
					pdf: Array.from(session.render_pdf(0, 'light')) as number[]
				};
			} finally { session.free(); }
		});
	}, cases.flatMap(source => [0, 1, 100].map(offset => [...cellNote(source, offset)])));
	for (const [index, source] of cases.entries()) {
		const original = results[index * 3];
		const translated = results[index * 3 + 1];
		const largeTranslation = results[index * 3 + 2];
		const expected = source.emitted_runs.runs;
		expect(original.constraint).toBe('Normal');
		expect(translated.constraint).toBe('Normal');
		expect(largeTranslation.constraint).toBe('Normal');
		expect(original.groups).toHaveLength(expected.length);
		expect(original.groups.map(group => group.source).join('')).toBe(source.text_utf8.replaceAll('\n', ''));
		const xShift = original.groups[0].spans[0].x[0] - float(expected[0].position_bits[0]);
		const yShift = original.groups[0].spans[0].y - float(expected[0].origin_bits[1]);
		for (const [run, group] of original.groups.entries()) {
			const positions = group.spans.flatMap(span => span.x);
			expect(positions).toHaveLength(expected[run].position_bits.length);
			for (const [glyph, x] of positions.entries()) expect(x).toBe(float(expected[run].position_bits[glyph]) + xShift);
			for (const span of group.spans) {
				expect(span.y).toBe(float(expected[run].origin_bits[1]) + yShift);
				expect(span.size).toBe(source.requested_font_size_bits === null ? 50 : float(source.requested_font_size_bits));
			}
		}
		expect(translated.groups).toHaveLength(original.groups.length);
		for (const [run, group] of translated.groups.entries()) {
			expect(group.source).toBe(original.groups[run].source);
			for (const [span, value] of group.spans.entries()) {
				expect(value.x).toEqual(original.groups[run].spans[span].x.map(x => x + 1));
				expect(value.y).toBe(original.groups[run].spans[span].y);
			}
		}
		if (source.name === 'consecutive-newlines') {
			const localX = float(expected[0].position_bits[0]);
			expect(Math.fround(localX + 100)).not.toBe(localX + 100);
		}
		for (const result of [original, translated, largeTranslation]) {
			expect(result.groups.map(group => group.source).join('')).toBe(source.text_utf8.replaceAll('\n', ''));
			expect(result.images).toBe(0);
			const pdf = await PDFDocument.load(new Uint8Array(result.pdf));
			const objects = pdf.context.enumerateIndirectObjects().map(([, value]) => value);
			const dictionaries = objects.map(value => value instanceof PDFRawStream ? value.dict.toString() : value.toString()).join('\n');
			const contents = objects.filter((value): value is PDFRawStream => value instanceof PDFRawStream).map(stream => Buffer.from(decodePDFRawStream(stream).decode()).toString('latin1')).join('\n');
			expect(dictionaries).toContain('/ToUnicode');
			expect(dictionaries).toContain('/FontFile2');
			expect(dictionaries).not.toMatch(/\/Subtype \/Image\b/);
			expect(contents).toMatch(/\b(?:Tj|TJ)\b/);
			const actualText = [...contents.matchAll(/\/ActualText\b/g)].map(match => {
				const value = PDFObjectParser.forBytes(Buffer.from(contents.slice(match.index! + match[0].length), 'latin1'), pdf.context).parseObject();
				if (!(value instanceof PDFHexString || value instanceof PDFString)) throw new Error('PDF ActualText must be a string.');
				return value.decodeText();
			});
			expect(actualText.join('')).toBe(source.text_utf8.replaceAll('\n', ''));
		}
	}
});
