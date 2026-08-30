import manifestText from '../../../../conformance/corpus.tsv?raw';

export const HF_DATASET_BASE_URL =
	'https://huggingface.co/datasets/twangodev/sdocx-compatibility/resolve/main';

export interface CorpusFixture {
	id: string;
	sdocx: string;
	sdocxSha256: string;
	referencePdf: string;
	referencePdfSha256: string;
	storedPages: number;
	visiblePages: number;
	title: string;
	minimumBodyCharacters: number;
	requiredText: string;
	textSections: number;
	hyperlinks: number;
	tables: number;
	codeBlocks: number;
	requiredLinkTarget: string;
	requiredTableText: string;
	requiredCodeText: string;
}

const COLUMN_COUNT = 17;
const SAFE_FILENAME = /^[a-zA-Z0-9][a-zA-Z0-9._-]*$/;
const SHA256 = /^[a-f0-9]{64}$/;

function integer(value: string, field: string, id: string): number {
	const parsed = Number(value);
	if (!Number.isSafeInteger(parsed) || parsed < 0) {
		throw new Error(`${id}: ${field} must be a non-negative integer`);
	}
	return parsed;
}

function filename(value: string, field: string, id: string): string {
	if (!SAFE_FILENAME.test(value)) throw new Error(`${id}: invalid ${field}`);
	return value;
}

function digest(value: string, field: string, id: string): string {
	if (!SHA256.test(value)) throw new Error(`${id}: invalid ${field}`);
	return value;
}

export function parseCorpusManifest(source: string): CorpusFixture[] {
	const fixtures: CorpusFixture[] = [];
	const ids = new Set<string>();

	for (const line of source.split(/\r?\n/)) {
		if (!line.trim() || line.startsWith('#')) continue;
		const fields = line.split('\t');
		if (fields.length !== COLUMN_COUNT) {
			throw new Error(`Invalid corpus row: expected ${COLUMN_COUNT} fields, got ${fields.length}`);
		}

		const id = filename(fields[0], 'fixture ID', fields[0] || 'fixture');
		if (ids.has(id)) throw new Error(`Duplicate corpus fixture: ${id}`);
		ids.add(id);

		fixtures.push({
			id,
			sdocx: filename(fields[1], 'SDOCX filename', id),
			sdocxSha256: digest(fields[2], 'SDOCX SHA-256', id),
			referencePdf: filename(fields[3], 'reference PDF filename', id),
			referencePdfSha256: digest(fields[4], 'reference PDF SHA-256', id),
			storedPages: integer(fields[5], 'stored page count', id),
			visiblePages: integer(fields[6], 'visible page count', id),
			title: fields[7],
			minimumBodyCharacters: integer(fields[8], 'minimum body character count', id),
			requiredText: fields[9],
			textSections: integer(fields[10], 'text section count', id),
			hyperlinks: integer(fields[11], 'hyperlink count', id),
			tables: integer(fields[12], 'table count', id),
			codeBlocks: integer(fields[13], 'code-block count', id),
			requiredLinkTarget: fields[14],
			requiredTableText: fields[15],
			requiredCodeText: fields[16]
		});
	}

	if (fixtures.length === 0) throw new Error('The bundled conformance corpus is empty.');
	return fixtures;
}

export function fixtureAssetUrl(filename: string): string {
	return `${HF_DATASET_BASE_URL}/${encodeURIComponent(filename)}?download=true`;
}

export const CORPUS_FIXTURES = parseCorpusManifest(manifestText);
