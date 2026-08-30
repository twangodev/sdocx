type JsonRecord = Record<string, unknown>;

function record(value: unknown): JsonRecord | undefined {
	return value !== null && typeof value === 'object' && !Array.isArray(value)
		? (value as JsonRecord)
		: undefined;
}

function array(value: unknown): unknown[] {
	return Array.isArray(value) ? value : [];
}

function textFromRichText(value: unknown): string | undefined {
	const object = record(value);
	return typeof object?.text === 'string' && object.text.trim() ? object.text.trim() : undefined;
}

function formatTimestamp(value: unknown): string | undefined {
	if (typeof value !== 'number' || !Number.isFinite(value)) return undefined;
	const date = new Date(value);
	return Number.isNaN(date.valueOf()) ? undefined : date.toLocaleString();
}

export interface InspectionView {
	title?: string;
	formatVersion?: string;
	created?: string;
	modified?: string;
	dimensions?: string;
	mediaCount: number;
	diagnostics: Array<{ code: string; message: string; entry?: string }>;
}

export function toInspectionView(inspection: unknown): InspectionView {
	const root = record(inspection) ?? {};
	const document = record(root.document) ?? root;
	const metadata = record(document.metadata) ?? {};
	const report = record(root.report) ?? {};
	const dimensions = array(metadata.page_dimensions);
	const format = metadata.format_version;
	const formatValue = Array.isArray(format) ? format[0] : format;

	return {
		title: textFromRichText(metadata.note_title),
		formatVersion:
			typeof formatValue === 'number' || typeof formatValue === 'string'
				? String(formatValue)
				: undefined,
		created: formatTimestamp(metadata.created_ms),
		modified: formatTimestamp(metadata.modified_ms),
		dimensions:
			dimensions.length >= 2 ? `${String(dimensions[0])} × ${String(dimensions[1])}` : undefined,
		mediaCount: array(metadata.media_assets).length,
		diagnostics: array(report.diagnostics).flatMap((item) => {
			const diagnostic = record(item);
			if (!diagnostic) return [];
			return [
				{
					code: String(diagnostic.code ?? 'Warning'),
					message: String(diagnostic.message ?? 'The parser reported a warning.'),
					entry:
						typeof diagnostic.archive_entry === 'string' ? diagnostic.archive_entry : undefined
				}
			];
		})
	};
}
