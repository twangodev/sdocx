import { describe, expect, it } from 'vitest';
import {
	CORPUS_FIXTURES,
	HF_DATASET_BASE_URL,
	fixtureAssetUrl,
	parseCorpusManifest
} from './manifest';

const digest = 'a'.repeat(64);

function row(id = 'fixture-one'): string {
	return [
		id,
		'note.sdocx',
		digest,
		'note.pdf',
		digest,
		'6',
		'5',
		'A title',
		'100',
		'Required body',
		'2',
		'1',
		'1',
		'1',
		'https://example.com',
		'Cell text',
		'Code text'
	].join('\t');
}

describe('conformance manifest', () => {
	it('bundles the locked compatibility fixture', () => {
		expect(CORPUS_FIXTURES).toHaveLength(1);
		expect(CORPUS_FIXTURES[0]).toMatchObject({
			id: '01-basic-formatting',
			storedPages: 6,
			visiblePages: 5,
			title: '01-basic-test'
		});
	});

	it('parses comments and all seventeen fixture fields', () => {
		const fixtures = parseCorpusManifest(`# ignored\n${row()}\n`);
		expect(fixtures[0]).toMatchObject({
			id: 'fixture-one',
			sdocxSha256: digest,
			referencePdfSha256: digest,
			minimumBodyCharacters: 100,
			requiredLinkTarget: 'https://example.com'
		});
	});

	it('rejects malformed rows, digests, and duplicate IDs', () => {
		expect(() => parseCorpusManifest('too\tfew')).toThrow(/17 fields/);
		expect(() => parseCorpusManifest(row().replace(digest, 'bad'))).toThrow(/SHA-256/);
		expect(() => parseCorpusManifest(`${row()}\n${row()}`)).toThrow(/Duplicate/);
	});

	it('builds a fixed Hugging Face asset URL', () => {
		expect(fixtureAssetUrl('fixture one.sdocx')).toBe(
			`${HF_DATASET_BASE_URL}/fixture%20one.sdocx?download=true`
		);
	});
});
