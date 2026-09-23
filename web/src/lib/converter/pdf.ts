import { jsPDF } from 'jspdf';
import { svg2pdf } from 'svg2pdf.js';

/** Convert pages sequentially, retaining strokes as PDF paths instead of rasterizing them. */
export async function createPdf(pages: AsyncIterable<string>): Promise<Blob> {
	let pdf: jsPDF | undefined;
	for await (const source of pages) {
		const document = new DOMParser().parseFromString(source, 'image/svg+xml');
		const svg = document.documentElement;
		if (document.querySelector('parsererror') || svg.localName !== 'svg') {
			throw new Error('The SVG page could not be read for PDF export.');
		}
		// Renderer dimensions are CSS pixels; PDF points are 1/72 inch.
		const width = Number(svg.getAttribute('width')) * 72 / 96;
		const height = Number(svg.getAttribute('height')) * 72 / 96;
		if (![width, height].every((value) => Number.isFinite(value) && value > 0 && value <= 14400)) {
			throw new Error('The page dimensions are outside the supported PDF range.');
		}
		const orientation = width > height ? 'landscape' : 'portrait';
		if (pdf) {
			pdf.addPage([width, height], orientation);
		} else {
			pdf = new jsPDF({ unit: 'pt', format: [width, height], orientation, compress: true, putOnlyUsedFonts: true });
			pdf.setProperties({ creator: 'sdocx' });
		}
		await svg2pdf(svg, pdf, { width, height, loadExternalStyleSheets: false, loadImages: /^data:image\// });
	}
	if (!pdf) throw new Error('The document has no pages to export.');
	return pdf.output('blob');
}
