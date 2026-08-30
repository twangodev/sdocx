import { describe, expect, it } from 'vitest';
import { LARGE_INPUT_BYTES, MAX_INPUT_BYTES, assertAcceptedFile, isLargeFile } from './protocol';

describe('converter input policy', () => {
	it('accepts a non-empty sdocx within the browser limit', () => {
		expect(() => assertAcceptedFile({ name: 'note.SDOCX', size: 1024 })).not.toThrow();
	});

	it('rejects unsupported, empty, and oversized inputs before allocating WASM memory', () => {
		expect(() => assertAcceptedFile({ name: 'note.zip', size: 1024 })).toThrow(/\.sdocx/);
		expect(() => assertAcceptedFile({ name: 'note.sdocx', size: 0 })).toThrow(/empty/);
		expect(() => assertAcceptedFile({ name: 'note.sdocx', size: MAX_INPUT_BYTES + 1 })).toThrow(
			/250 MiB/
		);
	});

	it('flags large files without rejecting them', () => {
		expect(isLargeFile({ size: LARGE_INPUT_BYTES })).toBe(false);
		expect(isLargeFile({ size: LARGE_INPUT_BYTES + 1 })).toBe(true);
	});
});
