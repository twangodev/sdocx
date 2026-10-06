import type { PageRenderReport, PdfRenderResult, SvgRenderResult } from './protocol';

function isRecord(value: unknown): value is Record<string, unknown> {
	return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function integerIn(value: unknown, min: number, max: number): value is number {
	return typeof value === 'number' && Number.isSafeInteger(value) && value >= min && value <= max;
}

function isPageReport(value: unknown): value is PageRenderReport {
	return isRecord(value)
		&& integerIn(value.page_index, 0, Number.MAX_SAFE_INTEGER)
		&& integerIn(value.source_page_index, 0, Number.MAX_SAFE_INTEGER)
		&& Array.isArray(value.text_diagnostics)
		&& value.text_diagnostics.every(issue => isRecord(issue)
			&& typeof issue.kind === 'string' && typeof issue.family === 'string'
			&& Array.isArray(issue.codepoints)
			&& issue.codepoints.every(point => integerIn(point, 0, 0xffffffff)))
		&& Array.isArray(value.object_diagnostics)
		&& value.object_diagnostics.every(issue => isRecord(issue)
			&& typeof issue.kind === 'string'
			&& integerIn(issue.anchor_utf16, -0x80000000, 0x7fffffff))
		&& Array.isArray(value.geometry_diagnostics)
		&& value.geometry_diagnostics.every(issue => isRecord(issue)
			&& typeof issue.kind === 'string' && typeof issue.object_uuid === 'string'
			&& (issue.source_offset === undefined || issue.source_offset === null
				|| integerIn(issue.source_offset, 0, Number.MAX_SAFE_INTEGER)))
		&& Array.isArray(value.paint_diagnostics)
		&& value.paint_diagnostics.every(issue => isRecord(issue)
			&& typeof issue.kind === 'string' && typeof issue.object_uuid === 'string'
			&& (issue.role === 'Fill' || issue.role === 'Outline')
			&& (issue.source_offset === undefined || issue.source_offset === null
				|| integerIn(issue.source_offset, 0, Number.MAX_SAFE_INTEGER)));
}

export function validateSvgResult(value: unknown, pageIndex: number): SvgRenderResult {
	if (!isPageReport(value) || typeof value.svg !== 'string' || value.page_index !== pageIndex) {
		throw new Error('The renderer returned an invalid SVG page report.');
	}
	return value as SvgRenderResult;
}

export function validatePdfResult(value: unknown, pageIndices: readonly number[]): PdfRenderResult {
	if (!isRecord(value) || !(value.bytes instanceof Uint8Array)
		|| !(value.bytes.buffer instanceof ArrayBuffer) || !Array.isArray(value.pages)
		|| value.pages.length !== pageIndices.length
		|| !value.pages.every((page, index) => isPageReport(page) && page.page_index === pageIndices[index])) {
		throw new Error('The renderer returned an invalid PDF page report.');
	}
	return value as PdfRenderResult;
}

export function pageReport({ svg: _svg, ...report }: SvgRenderResult): PageRenderReport {
	return report;
}

export function renderNoticeCount(reports: readonly PageRenderReport[]): number {
	return reports.reduce((count, report) => count + report.text_diagnostics.length + report.object_diagnostics.length + report.geometry_diagnostics.length + report.paint_diagnostics.length, 0);
}

export function diagnosticLabel(kind: string): string {
	return kind.replace(/([a-z0-9])([A-Z])/g, '$1 $2').replace(/[_-]+/g, ' ');
}

export function resultTransfers(value: unknown): Transferable[] {
	if (isRecord(value) && value.bytes instanceof Uint8Array && value.bytes.buffer instanceof ArrayBuffer) {
		return [value.bytes.buffer];
	}
	return [];
}
