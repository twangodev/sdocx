import { zipSync } from 'fflate';
import { join, zero, u16, u32, f32, f64, frame } from './wdoc';

export const retainedSource = '\u2066لا\u2069_123';

export function retainedPdfNote(): Buffer {
	const label = Buffer.from(retainedSource, 'utf16le');
	const family = Buffer.from('DejaVu Sans', 'utf8');
	const fontName = join(zero(8), u16(family.length + 1), family, zero(1));
	const spans = [
		join(u16(20), ...[3, 0, retainedSource.length, 1].map(u32), f32(20)),
		join(u16(20), ...[1, 0, retainedSource.length, 1, 0xff000000].map(u32)),
		join(u16(16 + fontName.length), ...[4, 0, retainedSource.length, 1].map(u32), fontName)
	];
	const common = join(u32(retainedSource.length), label, u32(spans.length), ...spans, u32(0), zero(16), zero(11));
	const base = frame(0, 1, join(u32(5500), u16(2), Buffer.from('tx'), zero(8), ...[10, 20, 310, 220].map(f64), zero(5)), f32(0));
	const payload = join(base, frame(6, 0), frame(7, 1, zero(0), join(u32(common.length), common)), frame(2, 0));
	const object = join(Buffer.from([2]), u16(0), u32(payload.length + 32), payload, zero(32));
	const header = join(zero(8), Buffer.from([1, 0, 5]), zero(5), ...[0, 400, 400, 0, 0].map(u32), u16(4), Buffer.from('text', 'utf16le'), zero(8), u32(5500), u32(4000));
	header.writeUInt32LE(header.length, 0);
	header.writeUInt32LE(header.length, 4);
	const layer = join(u32(20), zero(4), Buffer.from([2, 2, 0, 3, 0, 0, 0]), zero(5), u32(1), object, zero(32));
	return Buffer.from(zipSync({
		'text.page': join(header, u16(1), u16(0), layer, zero(32), Buffer.from('Page for SAMSUNG S-Pen SDK'))
	}));
}
