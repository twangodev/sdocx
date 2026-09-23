import { describe, expect, it } from 'vitest';
import { exportDetails, type ExportFormat } from './export-options';

const details = (format: ExportFormat, pageIndices: number[], count = 5) =>
	exportDetails({ format, pageIndices, pngScale: 1 }, count, 'notes');

describe('export output descriptions', () => {
	it('distinguishes whole documents, single pages and PDF subsets', () => {
		expect(details('pdf', [0, 1, 2, 3, 4]).filename).toBe('notes.pdf');
		expect(details('pdf', [3]).filename).toBe('notes-page-004.pdf');
		expect(details('pdf', [0, 2]).filename).toBe('notes-selected.pdf');
		expect(details('pdf', [0], 1).filename).toBe('notes.pdf');
	});
	it('uses ZIP only for multiple image pages', () => {
		for (const format of ['svg', 'png'] as const) {
			expect(details(format, [2]).filename).toBe(`notes-page-003.${format}`);
			expect(details(format, [0, 2]).filename).toBe(`notes-selected-${format}.zip`);
			expect(details(format, [0, 2]).button).toBe('Download ZIP');
			expect(details(format, [0, 1], 2).filename).toBe(`notes-${format}.zip`);
			expect(details(format, [0], 1).button).toBe(`Download ${format.toUpperCase()}`);
		}
	});
	it('describes document-wide data formats without promising a PDF in the archive', () => {
		expect(details('json', []).filename).toBe('notes.json');
		expect(details('everything', []).description).toBe('All pages · SVG, PNG and JSON in a ZIP');
	});
});
