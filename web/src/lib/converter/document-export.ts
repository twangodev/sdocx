import type { ConverterClientPort, ProgressListener } from './client';
import type { ExportRequest } from './export-options';
import { createExportManifest, createZip, pageFilename, svgToPng, textBytes } from './files';
import { pendingProgress, type WorkListener } from './progress';
import type { ColorMode, PageRenderReport } from './protocol';
import { pageReport } from './render-reports';

interface ExportDocumentOptions {
	client: Pick<ConverterClientPort, 'exportPdf' | 'exportJson' | 'renderPage'>;
	format: ExportRequest['format'];
	pngScale: ExportRequest['pngScale'];
	indices: number[];
	pageCount: number;
	colorMode: ColorMode;
	stem: string;
	sourceName: string;
	assertCurrent: () => void;
	onProgress: ProgressListener;
	onStatus: (status: string) => void;
	onWork: WorkListener;
}

interface ExportDocumentResult {
	blob: Blob;
	reports: PageRenderReport[];
}

export async function buildDocumentExport({
	client, format, pngScale, indices, pageCount, colorMode, stem, sourceName,
	assertCurrent, onProgress, onStatus, onWork
}: ExportDocumentOptions): Promise<ExportDocumentResult> {
	const reports: PageRenderReport[] = [];
	if (!indices.length || indices.some((index) => !Number.isInteger(index) || index < 0 || index >= pageCount)) {
		throw new Error('Select valid pages to export.');
	}
	let blob: Blob;
	if (format === 'pdf') {
		onStatus('Generating PDF');
		const result = await client.exportPdf(indices, colorMode, onProgress);
		assertCurrent();
		reports.push(...result.pages);
		blob = new Blob([result.bytes], { type: 'application/pdf' });
	} else if (format === 'json') {
		onWork(pendingProgress('inspecting'));
		const json = await client.exportJson();
		assertCurrent();
		blob = new Blob([json], { type: 'application/json' });
	} else if (format !== 'everything' && indices.length === 1) {
		const result = await client.renderPage(indices[0], colorMode, onProgress);
		assertCurrent();
		reports.push(pageReport(result));
		if (format === 'png') onWork(pendingProgress('encoding'));
		blob = format === 'png' ? await svgToPng(result.svg, pngScale) : new Blob([result.svg], { type: 'image/svg+xml' });
	} else {
		async function* entries() {
			if (format === 'everything') {
				const json = await client.exportJson();
				assertCurrent();
				yield { name: 'document.json', bytes: textBytes(json) };
				const manifest = createExportManifest(sourceName, pageCount, colorMode, pngScale);
				yield { name: 'manifest.json', bytes: textBytes(JSON.stringify(manifest, null, 2)) };
			}
			for (const [position, index] of indices.entries()) {
				assertCurrent();
				onStatus(`Rendering page ${position + 1} of ${indices.length}`);
				const result = await client.renderPage(index, colorMode, onProgress);
				assertCurrent();
				reports.push(pageReport(result));
				if (format === 'svg' || format === 'everything') {
					yield { name: pageFilename(stem, index, 'svg'), bytes: textBytes(result.svg) };
				}
				if (format === 'png' || format === 'everything') {
					onWork(pendingProgress('encoding'));
					onStatus(`Rasterizing page ${position + 1} of ${indices.length}`);
					const png = await svgToPng(result.svg, pngScale);
					assertCurrent();
					const bytes = new Uint8Array(await png.arrayBuffer());
					assertCurrent();
					yield { name: pageFilename(stem, index, 'png'), bytes };
				}
				onWork(pendingProgress('packaging'));
			}
		}
		blob = await createZip(entries());
	}
	assertCurrent();
	return { blob, reports };
}
