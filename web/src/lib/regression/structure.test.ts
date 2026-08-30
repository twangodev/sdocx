import { describe, expect, it } from 'vitest';
import type { CorpusFixture } from './manifest';
import { hyperlinkTarget, runStructuralChecks } from './structure';

const target = 'https://example.com/markdown-test';

const fixture: CorpusFixture = {
	id: 'fixture',
	sdocx: 'fixture.sdocx',
	sdocxSha256: 'a'.repeat(64),
	referencePdf: 'fixture.pdf',
	referencePdfSha256: 'b'.repeat(64),
	storedPages: 6,
	visiblePages: 5,
	title: '01-basic-test',
	minimumBodyCharacters: 20,
	requiredText: 'Final sentence',
	textSections: 2,
	hyperlinks: 1,
	tables: 1,
	codeBlocks: 1,
	requiredLinkTarget: target,
	requiredTableText: 'Alpha',
	requiredCodeText: 'Markdown code fence'
};

function hyperlinkPayload(value: string): number[] {
	const units = Array.from(value, (character) => character.charCodeAt(0));
	const header = new Uint8Array(12);
	const view = new DataView(header.buffer);
	view.setUint32(0, 3, true);
	view.setUint32(8, units.length, true);
	return [...header, ...units.flatMap((unit) => [unit & 0xff, unit >>> 8])];
}

function passingInspection(): unknown {
	return {
		document: {
			pages: Array.from({ length: 6 }, () => ({})),
			metadata: {
				note_title: { text: '01-basic-test' },
				note_text: {
					text: 'A sufficiently long body. Final sentence.',
					text_sections: [{}, {}],
					spans: [{ kind: 'Hyperlink', payload: hyperlinkPayload(target) }],
					object_spans: [
						{
							content: {
								Table: { rows: [{ cells: [{ content: { text: 'Alpha cell' } }] }] }
							}
						},
						{ content: { CodeBlock: { body: { text: 'Markdown code fence' } } } }
					]
				}
			}
		},
		stored_page_count: 6,
		report: { diagnostics: [] }
	};
}

describe('strict conformance checks', () => {
	it('decodes the exact UTF-16LE hyperlink payload used by the Rust parser', () => {
		expect(hyperlinkTarget({ kind: 'Hyperlink', payload: hyperlinkPayload(target) })).toBe(target);
		expect(hyperlinkTarget({ kind: 'ForegroundColor', payload: [] })).toBeUndefined();
		expect(hyperlinkTarget({ kind: 'Hyperlink', payload: [0, 1] })).toBeUndefined();
	});

	it('passes when every locked structural expectation matches', () => {
		const result = runStructuralChecks(fixture, passingInspection(), 5);
		expect(result.passed).toBe(true);
		expect(result.checks).toHaveLength(13);
		expect(result.checks.every((check) => check.passed)).toBe(true);
	});

	it('reports individual structural mismatches', () => {
		const inspection = passingInspection() as {
			document: { metadata: { note_title: { text: string } } };
			report: { diagnostics: unknown[] };
		};
		inspection.document.metadata.note_title.text = 'Wrong title';
		inspection.report.diagnostics.push({ message: 'warning' });

		const result = runStructuralChecks(fixture, inspection, 4);
		expect(result.passed).toBe(false);
		expect(result.checks.filter((check) => !check.passed).map((check) => check.id)).toEqual([
			'visible-pages',
			'title',
			'diagnostics'
		]);
	});
});
