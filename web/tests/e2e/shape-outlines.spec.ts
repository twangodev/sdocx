import { expect, test } from '../fixtures/browser';
import { shapeOutlineNote } from '../fixtures/pdf-note';

test('Rust outline styles reach browser SVG and PDF with vector cleanup clips', async ({ page }) => {
	await page.route('https://rybbit.twango.dev/**', route => route.fulfill({ body: '' }));
	await page.goto('/');
	await page.locator('input[type=file]').setInputFiles({ name: 'outlines.sdocx', mimeType: 'application/zip', buffer: shapeOutlineNote() });
	await expect(page.getByAltText('Rendered preview of page 1')).toBeVisible();
	const results = await page.evaluate(async bytes => {
		const wasm = await import(`${location.origin}/wasm/sdocx_wasm.js`);
		await wasm.default();
		const session = new wasm.DocumentSession(new Uint8Array(bytes));
		try {
			return ['light', 'dark'].map(mode => {
				const rendered = session.render_svg_detailed(0, mode);
				const xml = new DOMParser().parseFromString(rendered.svg, 'image/svg+xml');
				const pdf = session.render_pdf_pages_detailed(new Uint32Array([0]), mode);
				return { parseError: Boolean(xml.querySelector('parsererror')),
					dashes: [...xml.querySelectorAll('[stroke-dasharray]')].map(node => node.getAttribute('stroke-dasharray')),
					clips: xml.querySelectorAll('clipPath path[clip-rule="evenodd"]').length,
					circles: xml.querySelectorAll('g[transform] circle').length,
					images: xml.querySelectorAll('image').length,
					svgIssues: rendered.paint_diagnostics,
					pdfIssues: pdf.pages[0].paint_diagnostics,
					pdfHeader: new TextDecoder().decode(pdf.bytes.slice(0, 5)) };
			});
		} finally { session.free(); }
	}, [...shapeOutlineNote()]);
	for (const result of results) {
		expect(result).toMatchObject({ parseError: false, clips: 3, circles: 2, images: 0, svgIssues: [], pdfIssues: [], pdfHeader: '%PDF-' });
		expect(result.dashes).toHaveLength(5);
		for (const pattern of result.dashes) expect(pattern?.split(/[ ,]+/).map(Number)).toEqual([28, 10.5, 3.5, 10.5, 3.5, 10.5]);
	}
});
