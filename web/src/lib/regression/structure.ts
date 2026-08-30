import type { CorpusFixture } from './manifest';

type JsonRecord = Record<string, unknown>;

export interface StructuralCheck {
	id: string;
	label: string;
	expected: string | number | boolean;
	actual: string | number | boolean;
	passed: boolean;
}

export interface StructuralResult {
	passed: boolean;
	checks: StructuralCheck[];
}

function record(value: unknown): JsonRecord | undefined {
	return value !== null && typeof value === 'object' && !Array.isArray(value)
		? (value as JsonRecord)
		: undefined;
}

function array(value: unknown): unknown[] {
	return Array.isArray(value) ? value : [];
}

function text(value: unknown): string | undefined {
	return typeof value === 'string' ? value : undefined;
}

function richText(value: unknown): string | undefined {
	return text(record(value)?.text);
}

function unicodeLength(value: string): number {
	return Array.from(value).length;
}

function bytes(value: unknown): number[] {
	if (value instanceof Uint8Array) return Array.from(value);
	return array(value).flatMap((item) =>
		typeof item === 'number' && Number.isInteger(item) && item >= 0 && item <= 255 ? [item] : []
	);
}

function u32le(value: number[], offset: number): number | undefined {
	if (offset + 4 > value.length) return undefined;
	return (
		(value[offset] |
			(value[offset + 1] << 8) |
			(value[offset + 2] << 16) |
			(value[offset + 3] << 24)) >>>
		0
	);
}

export function hyperlinkTarget(span: unknown): string | undefined {
	const object = record(span);
	if (object?.kind !== 'Hyperlink') return undefined;
	const payload = bytes(object.payload);
	const length = u32le(payload, 8);
	if (length === undefined || 12 + length * 2 > payload.length) return undefined;
	const codeUnits: number[] = [];
	for (let index = 0; index < length; index += 1) {
		const offset = 12 + index * 2;
		codeUnits.push(payload[offset] | (payload[offset + 1] << 8));
	}
	if (codeUnits.length === 0) return undefined;
	let output = '';
	for (let index = 0; index < codeUnits.length; index += 4096) {
		output += String.fromCharCode(...codeUnits.slice(index, index + 4096));
	}
	return output;
}

function contentVariant(span: unknown, name: 'Table' | 'CodeBlock'): JsonRecord | undefined {
	const content = record(record(span)?.content);
	return record(content?.[name]);
}

function tableContains(table: JsonRecord, required: string): boolean {
	return array(table.rows).some((row) =>
		array(record(row)?.cells).some((cell) => richText(record(cell)?.content)?.includes(required) ?? false)
	);
}

function codeBlockContains(codeBlock: JsonRecord, required: string): boolean {
	return richText(codeBlock.body)?.includes(required) ?? false;
}

function check(
	id: string,
	label: string,
	expected: string | number | boolean,
	actual: string | number | boolean
): StructuralCheck {
	return { id, label, expected, actual, passed: Object.is(expected, actual) };
}

export function runStructuralChecks(
	fixture: CorpusFixture,
	inspection: unknown,
	visiblePageCount: number
): StructuralResult {
	const root = record(inspection) ?? {};
	const document = record(root.document) ?? {};
	const metadata = record(document.metadata) ?? {};
	const noteBody = richText(metadata.note_text) ?? '';
	const noteTitle = richText(metadata.note_title) ?? '';
	const flow = record(metadata.note_text) ?? {};
	const spans = array(flow.spans);
	const objectSpans = array(flow.object_spans);
	const hyperlinks = spans.filter((span) => record(span)?.kind === 'Hyperlink');
	const tables = objectSpans.flatMap((span) => {
		const value = contentVariant(span, 'Table');
		return value ? [value] : [];
	});
	const codeBlocks = objectSpans.flatMap((span) => {
		const value = contentVariant(span, 'CodeBlock');
		return value ? [value] : [];
	});
	const diagnostics = array(record(root.report)?.diagnostics);
	const storedPageCount =
		typeof root.stored_page_count === 'number' ? root.stored_page_count : array(document.pages).length;

	const checks = [
		check('stored-pages', 'Stored page count', fixture.storedPages, storedPageCount),
		check('visible-pages', 'Visible page count', fixture.visiblePages, visiblePageCount),
		check('title', 'Note title', fixture.title, noteTitle),
		check(
			'body-length',
			'Minimum body characters',
			true,
			unicodeLength(noteBody) >= fixture.minimumBodyCharacters
		),
		check('required-text', 'Required body text', true, noteBody.includes(fixture.requiredText)),
		check('text-sections', 'Text section count', fixture.textSections, array(flow.text_sections).length),
		check('hyperlinks', 'Hyperlink count', fixture.hyperlinks, hyperlinks.length),
		check(
			'link-target',
			'Required hyperlink target',
			true,
			hyperlinks.some((span) => hyperlinkTarget(span) === fixture.requiredLinkTarget)
		),
		check('tables', 'Table count', fixture.tables, tables.length),
		check(
			'table-text',
			'Required table text',
			true,
			tables.some((table) => tableContains(table, fixture.requiredTableText))
		),
		check('code-blocks', 'Code-block count', fixture.codeBlocks, codeBlocks.length),
		check(
			'code-text',
			'Required code-block text',
			true,
			codeBlocks.some((codeBlock) => codeBlockContains(codeBlock, fixture.requiredCodeText))
		),
		check('diagnostics', 'Parser diagnostics', 0, diagnostics.length)
	];

	return { passed: checks.every((item) => item.passed), checks };
}
