import { readFile } from 'node:fs/promises';
import { expect, test } from '../fixtures/browser';
import { shapeGradientNote } from '../fixtures/pdf-note';
import type { ConverterProgress } from '../../src/lib/converter/protocol';

declare global {
	interface Window {
		decodeProgressCapture: {
			nextWorker: number;
			messages: Array<{ type: string; id: number; worker: number; event?: ConverterProgress }>;
			bars: Array<{ label: string | null; value: number }>;
		};
	}
}

test('shows live Rust counters before worker results during import, preview and PDF export', async ({ page }) => {
	await page.addInitScript(() => {
		window.decodeProgressCapture = { nextWorker: 0, messages: [], bars: [] };
		const NativeWorker = window.Worker;
		window.Worker = class extends NativeWorker {
			constructor(url: string | URL, options?: WorkerOptions) {
				super(url, options);
				const worker = ++window.decodeProgressCapture.nextWorker;
				this.addEventListener('message', (message) => {
					const data = message.data;
					window.decodeProgressCapture.messages.push({ type: data.type, id: data.id, worker, event: data.type === 'progress' ? data : undefined });
					queueMicrotask(() => queueMicrotask(() => {
						for (const bar of document.querySelectorAll('progress')) {
							if (bar.hasAttribute('value')) window.decodeProgressCapture.bars.push({ label: bar.getAttribute('aria-label'), value: bar.value });
						}
					}));
				});
			}
		};
	});
	await page.route('https://rybbit.twango.dev/**', route => route.fulfill({ body: '' }));
	await page.goto('/');
	await expect(page.getByRole('button', { name: 'Import notes', exact: true })).toBeEnabled();
	await page.locator('input[type=file]').setInputFiles({ name: 'progress.sdocx', mimeType: 'application/zip', buffer: shapeGradientNote() });
	await expect(page.getByAltText('Rendered preview of page 1')).toBeVisible();
	await expect(page.getByRole('button', { name: 'Export document', exact: true })).toBeEnabled();
	await page.getByRole('button', { name: 'Export document', exact: true }).click();
	const dialog = page.getByRole('dialog');
	const download = page.waitForEvent('download');
	await dialog.getByRole('button', { name: 'Download PDF', exact: true }).click();
	expect((await readFile((await (await download).path())!)).subarray(0, 5).toString()).toBe('%PDF-');
	const capture = await page.evaluate(() => window.decodeProgressCapture);
	const updates = capture.messages.flatMap(message => message.event ? [message.event] : []);
	expect(updates).toEqual(expect.arrayContaining([
		expect.objectContaining({ operation: 'load', progress: expect.objectContaining({ stage: 'objects', completed: 1 }) }),
		expect.objectContaining({ operation: 'renderPage', progress: expect.objectContaining({ stage: 'rendering', completed: 1 }) }),
		expect.objectContaining({ operation: 'exportPdf', progress: { stage: 'writingPdf', completed: 1, total: 1 } })
	]));
	for (const [index, message] of capture.messages.entries()) {
		if (!message.event) continue;
		const resultIndex = capture.messages.findIndex((result, resultIndex) => resultIndex > index && result.worker === message.worker && result.id === message.id && result.type === 'result');
		expect(resultIndex).toBeGreaterThan(index);
	}

	expect(capture.bars.some(bar => bar.label === 'Decoding objects' && bar.value > 0 && bar.value < 1)).toBe(true);
	await expect(dialog.getByText('Download started', { exact: true })).toBeVisible();
	await expect(dialog.getByRole('progressbar')).toHaveCount(0);
});

test('WASM observers preserve output and reject recursive rendering without invalidating the session', async ({ page }) => {
	await page.route('https://rybbit.twango.dev/**', route => route.fulfill({ body: '' }));
	await page.goto('/');
	const result = await page.evaluate(async bytes => {
		const module = await import(`${location.origin}/wasm/sdocx_wasm.js`);
		await module.default();
		const decoded: Array<{ stage: string; completed: number; total: number | null }> = [];
		const session = module.DocumentSession.create_with_progress(new Uint8Array(bytes), (progress: typeof decoded[number]) => decoded.push(progress));
		try {
			const normal = session.render_svg_detailed(0, 'auto');
			let recursiveError = '';
			const reported = session.render_svg_detailed_with_progress(0, 'auto', (progress: typeof decoded[number]) => {
				if (progress.stage === 'rendering' && !recursiveError) {
					try { session.render_svg(0, 'auto'); } catch (error) { recursiveError = (error as Error).message; }
				}
			});
			const throwing = session.render_svg_detailed_with_progress(0, 'auto', () => { throw new Error('Observer failed'); });
			const normalPdf = session.render_pdf_pages_detailed(new Uint32Array([0, 0]), 'auto');
			const writing: typeof decoded = [];
			const reportedPdf = session.render_pdf_pages_detailed_with_progress(new Uint32Array([0, 0]), 'auto', (progress: typeof decoded[number]) => writing.push(progress));
			return { sameSvg: normal.svg === reported.svg && normal.svg === throwing.svg,
				samePdf: normalPdf.bytes.length === reportedPdf.bytes.length && normalPdf.bytes.every((byte: number, index: number) => byte === reportedPdf.bytes[index]),
				recursiveError, decoded, writing: writing.filter(progress => progress.stage === 'writingPdf'),
				finalizing: writing.at(-1) };
		} finally { session.free(); }
	}, [...shapeGradientNote()]);
	expect(result.sameSvg).toBe(true);
	expect(result.samePdf).toBe(true);
	expect(result.recursiveError).toMatch(/already in progress/);
	expect(result.decoded).toContainEqual({ stage: 'archive', completed: 0, total: null });
	expect(result.writing.map(progress => progress.completed)).toEqual([0, 1, 2]);
	expect(result.finalizing).toEqual({ stage: 'finalizing', completed: 0, total: null });
});
