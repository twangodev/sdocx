export const join = (...parts: Uint8Array[]) => Buffer.concat(parts);
export const zero = (length: number) => Buffer.alloc(length);
export const u16 = (value: number) => { const bytes = Buffer.alloc(2); bytes.writeUInt16LE(value); return bytes; };
export const u32 = (value: number) => { const bytes = Buffer.alloc(4); bytes.writeUInt32LE(value); return bytes; };
export const f32 = (value: number) => { const bytes = Buffer.alloc(4); bytes.writeFloatLE(value); return bytes; };
export const f64 = (value: number) => { const bytes = Buffer.alloc(8); bytes.writeDoubleLE(value); return bytes; };

export function frame(kind: number, fields: number, fixed = zero(0), flexible = zero(0), properties = kind === 0 ? 8 : 0) {
	const offset = 18 + fixed.length;
	return join(u32(offset + flexible.length), u16(kind), u32(offset), Buffer.from([2]), u16(properties), Buffer.from([4]), u32(fields), fixed, flexible);
}
