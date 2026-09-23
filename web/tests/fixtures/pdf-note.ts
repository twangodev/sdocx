import { zipSync } from 'fflate';

// Synthetic WDoc records following crates/sdocx/tests/{support,structural_strokes,
// structural_text_boxes}. No external corpus or parser mocks are required.
const join = (...parts: Uint8Array[]) => Buffer.concat(parts);
const u16 = (n: number) => { const b = Buffer.alloc(2); b.writeUInt16LE(n); return b; };
const u32 = (n: number) => { const b = Buffer.alloc(4); b.writeUInt32LE(n); return b; };
const f32 = (n: number) => { const b = Buffer.alloc(4); b.writeFloatLE(n); return b; };
const f64 = (n: number) => { const b = Buffer.alloc(8); b.writeDoubleLE(n); return b; };
const zero = (n: number) => Buffer.alloc(n);

function frame(kind: number, properties: number, fields: number, fixed = zero(0), flexible = zero(0)) {
	const offset = 18 + fixed.length;
	return join(u32(offset + flexible.length), u16(kind), u32(offset), Buffer.from([2]), u16(properties), Buffer.from([4]), u32(fields), fixed, flexible);
}

function base() {
	return frame(0, 8, 1, join(u32(5500), u16(2), Buffer.from('id'), zero(8), ...[10, 20, 210, 160].map(f64), zero(5)), f32(0));
}

function object(kind: number, payload: Buffer) {
	return join(Buffer.from([kind]), u16(0), u32(payload.length + 32), payload, zero(32));
}

function stroke(index: number) {
	const points = index === 0 ? [10, 20, 100, 120, 200, 20] : [30, 40, 150, 80, 250, 40];
	const channels = join(...points.map(f64), ...[0.5, 0.75, 0.5].map(f32), ...[0, 10, 20].map(u32));
	return object(1, join(base(), frame(1, 0, 2, join(u16(3), channels, Buffer.from([1, 0])), f32(4))));
}

function text(index: number) {
	const label = Buffer.from(`Page ${index + 1} · café Ω`, 'utf16le');
	const common = join(u32(label.length / 2), label, zero(8), ...[2, 3, 4, 5].map(f32), zero(11));
	return object(2, join(base(), frame(6, 0, 0), frame(7, 0, 1, zero(0), join(u32(common.length), common)), frame(2, 0, 0)));
}

function page(index: number, width: number, height: number) {
	const header = join(zero(8), Buffer.from([1, 0, 5]), zero(5), ...[0, width, height, 0, 0].map(u32), u16(4), Buffer.from(index === 0 ? 'one1' : 'two2', 'utf16le'), zero(8), u32(5500), u32(4000));
	header.writeUInt32LE(header.length, 0);
	header.writeUInt32LE(header.length, 4);
	const layer = join(u32(20), zero(4), Buffer.from([2, 2, 0, 3, 0, 0, 0]), zero(5), u32(2), stroke(index), text(index), zero(32));
	return join(header, u16(1), u16(0), layer, zero(32), Buffer.from('Page for SAMSUNG S-Pen SDK'));
}

export function pdfNote(oversized = false): Buffer {
	return Buffer.from(zipSync({
		'one1.page': page(0, oversized ? 20000 : 400, 800),
		'two2.page': page(1, 800, 400)
	}));
}
