import { pageFilename } from './files';

export type ExportFormat = 'pdf' | 'png' | 'svg' | 'json' | 'everything';
export interface ExportRequest {
	format: ExportFormat;
	pageIndices: number[];
	pngScale: 1 | 2;
}

export function exportDetails(request: ExportRequest, pageCount: number, stem: string) {
	const { format, pageIndices } = request;
	const wholeDocument = format === 'json' || format === 'everything';
	const count = wholeDocument ? pageCount : pageIndices.length;
	const all = count === pageCount;
	const zip = format === 'everything' || ((format === 'png' || format === 'svg') && count > 1);
	let filename: string;
	if (format === 'json') filename = `${stem}.json`;
	else if (format === 'everything') filename = `${stem}-everything.zip`;
	else if (zip) filename = `${stem}${all ? '' : '-selected'}-${format}.zip`;
	else if (format === 'pdf' && all) filename = `${stem}.pdf`;
	else if (count === 1) filename = pageFilename(stem, pageIndices[0], format);
	else filename = `${stem}-selected.pdf`;
	const description = format === 'everything' ? 'All pages · SVG, PNG and JSON in a ZIP'
		: format === 'json' ? 'Whole document · JSON'
		: zip ? `${count} ${format.toUpperCase()} files in a ZIP`
		: `${count} ${count === 1 ? 'page' : 'pages'} · ${format.toUpperCase()}`;
	return { filename, description, button: `Download ${zip ? 'ZIP' : format.toUpperCase()}` };
}
