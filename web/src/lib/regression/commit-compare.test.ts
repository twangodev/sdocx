import { describe, expect, it } from 'vitest';
import type { RenderedDocument } from './commit-protocol';
import { compareRenderedDocuments } from './commit-compare';
import { parseCorpusManifest } from './manifest';

const fixture = parseCorpusManifest(
	'fixture\tfixture.sdocx\t' +
		'a'.repeat(64) +
		'\tfixture.pdf\t' +
		'b'.repeat(64) +
		'\t0\t1\t\t0\t\t0\t0\t0\t0\t\t\t'
)[0];

function document(svg = '<svg viewBox="0 0 10 20"></svg>'): RenderedDocument {
	return {
		pageCount: 1,
		inspection: { document: { pages: [], metadata: {} }, stored_page_count: 0, report: {} },
		pages: [{ width: 10, height: 20, svg }]
	};
}

describe('commit renderer comparison', () => {
	it('keeps byte-identical documents unchanged', () => {
		const result = compareRenderedDocuments(fixture, document(), document());
		expect(result.changed).toBe(false);
		expect(result.changedPageCount).toBe(0);
		expect(result.structuralDiffs.every((item) => !item.changed)).toBe(true);
	});

	it('reports visual and structural changes independently', () => {
		const left = document();
		const right = document('<svg viewBox="0 0 10 20"><path /></svg>');
		right.pageCount = 2;
		const result = compareRenderedDocuments(fixture, left, right);
		expect(result.changed).toBe(true);
		expect(result.changedPageCount).toBe(1);
		expect(result.structuralDiffs.find((item) => item.id === 'visible-pages')?.changed).toBe(true);
	});

	it('reports pages that exist on only one side', () => {
		const right = document();
		right.pages.push({ width: 10, height: 20, svg: '<svg></svg>' });
		const result = compareRenderedDocuments(fixture, document(), right);
		expect(result.pages[1].status).toBe('right-only');
	});
});
