import { describe, expect, it } from 'vitest';
import { resultTransfers, validatePdfResult, validateSvgResult } from './render-reports';

function page(page_index: number) {
	return {
		page_index,
		text_diagnostics: [{ kind: 'FutureFontNotice', family: 'Example', codepoints: [0x1f600], detail: { fallback: true } }],
		object_diagnostics: [{ kind: 'FutureObjectNotice', anchor_utf16: -1, source: 'retained' }]
	};
}

describe('render report boundary', () => {
	it('preserves unfamiliar kinds and additional fields without an enum whitelist', () => {
		const result = { ...page(2), svg: '<svg/>', future_summary: { count: 1 } };
		expect(validateSvgResult(result, 2)).toBe(result);
		expect(validateSvgResult(result, 2).text_diagnostics[0].detail).toEqual({ fallback: true });
		expect(() => validateSvgResult(result, 1)).toThrow(/invalid SVG/);
		expect(() => validateSvgResult({ ...result, text_diagnostics: [{ kind: 'FutureNotice', family: '', codepoints: ['65'] }] }, 2)).toThrow(/invalid SVG/);
		expect(() => validateSvgResult({ ...result, object_diagnostics: [{ kind: 'FutureNotice', anchor_utf16: 0.5 }] }, 2)).toThrow(/invalid SVG/);
	});

	it('keeps PDF report order and repeats and transfers the nested owned bytes', () => {
		const result = { bytes: new Uint8Array([37, 80, 68, 70]), pages: [page(1), page(0), page(1)] };
		expect(validatePdfResult(result, [1, 0, 1])).toBe(result);
		expect(() => validatePdfResult(result, [0, 1, 1])).toThrow(/invalid PDF/);
		expect(() => validatePdfResult({ ...result, pages: result.pages.slice(0, 2) }, [1, 0, 1])).toThrow(/invalid PDF/);
		const transferred = structuredClone(result, { transfer: resultTransfers(result) });
		expect(result.bytes.byteLength).toBe(0);
		expect(Array.from(transferred.bytes)).toEqual([37, 80, 68, 70]);
		expect(transferred.pages.map(report => report.page_index)).toEqual([1, 0, 1]);
	});
});
