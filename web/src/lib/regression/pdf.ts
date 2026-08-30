import * as pdfjs from 'pdfjs-dist';
import pdfWorkerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url';
import type { ConverterClient } from '../converter/client';
import { comparePixels, compareSourceDimensions, type PageVisualMetrics } from './metrics';

pdfjs.GlobalWorkerOptions.workerSrc = pdfWorkerUrl;

interface PdfViewport {
	width: number;
	height: number;
}

interface PdfRenderTask {
	promise: Promise<void>;
	cancel: () => void;
}

interface PdfPageProxy {
	getViewport(options: { scale: number }): PdfViewport;
	render(options: {
		canvas: HTMLCanvasElement;
		canvasContext: CanvasRenderingContext2D;
		viewport: PdfViewport;
	}): PdfRenderTask;
}

interface PdfDocumentProxy {
	numPages: number;
	getPage(pageNumber: number): Promise<PdfPageProxy>;
	destroy(): Promise<void>;
}

export interface ComparisonPage {
	pageIndex: number;
	actualSvgUrl: string;
	actualRasterUrl: string;
	referenceUrl: string;
	heatmapUrl: string;
	actualRasterPng: Blob;
	referencePng: Blob;
	heatmapPng: Blob;
	metrics: PageVisualMetrics;
}

export interface ComparisonSet {
	pages: ComparisonPage[];
	referencePageCount: number;
}

interface RenderedCanvas {
	canvas: HTMLCanvasElement;
	sourceWidth: number;
	sourceHeight: number;
}

function throwIfAborted(signal: AbortSignal): void {
	if (signal.aborted) {
		throw signal.reason ?? new DOMException('The regression run was cancelled.', 'AbortError');
	}
}

function canvas(width: number, height: number): HTMLCanvasElement {
	const output = document.createElement('canvas');
	output.width = Math.max(1, Math.round(width));
	output.height = Math.max(1, Math.round(height));
	return output;
}

function context2d(target: HTMLCanvasElement): CanvasRenderingContext2D {
	const context = target.getContext('2d', { willReadFrequently: true });
	if (!context) throw new Error('Canvas rendering is unavailable in this browser.');
	return context;
}

async function renderPdfPage(
	page: PdfPageProxy,
	signal: AbortSignal
): Promise<RenderedCanvas> {
	const natural = page.getViewport({ scale: 1 });
	const scale = Math.min(2.5, Math.max(1, 1200 / natural.width));
	const viewport = page.getViewport({ scale });
	const output = canvas(viewport.width, viewport.height);
	const context = context2d(output);
	context.fillStyle = '#fff';
	context.fillRect(0, 0, output.width, output.height);
	const task = page.render({ canvas: output, canvasContext: context, viewport });
	const cancel = () => task.cancel();
	signal.addEventListener('abort', cancel, { once: true });
	try {
		await task.promise;
		throwIfAborted(signal);
		return { canvas: output, sourceWidth: natural.width, sourceHeight: natural.height };
	} finally {
		signal.removeEventListener('abort', cancel);
	}
}

async function loadSvg(svg: string, signal: AbortSignal): Promise<HTMLImageElement> {
	const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
	try {
		const image = new Image();
		await new Promise<void>((resolve, reject) => {
			const abort = () => reject(signal.reason ?? new DOMException('Cancelled', 'AbortError'));
			image.onload = () => {
				signal.removeEventListener('abort', abort);
				resolve();
			};
			image.onerror = () => {
				signal.removeEventListener('abort', abort);
				reject(new Error('The generated SVG could not be rasterized.'));
			};
			signal.addEventListener('abort', abort, { once: true });
			image.src = url;
		});
		return image;
	} finally {
		URL.revokeObjectURL(url);
	}
}

async function rasterizeSvg(
	svg: string,
	width: number,
	height: number,
	signal: AbortSignal
): Promise<RenderedCanvas> {
	const image = await loadSvg(svg, signal);
	throwIfAborted(signal);
	const output = canvas(width, height);
	const context = context2d(output);
	context.fillStyle = '#fff';
	context.fillRect(0, 0, width, height);
	const scale = Math.min(width / image.naturalWidth, height / image.naturalHeight);
	const drawWidth = image.naturalWidth * scale;
	const drawHeight = image.naturalHeight * scale;
	context.drawImage(image, (width - drawWidth) / 2, (height - drawHeight) / 2, drawWidth, drawHeight);
	return {
		canvas: output,
		sourceWidth: image.naturalWidth,
		sourceHeight: image.naturalHeight
	};
}

function heatmapCanvas(width: number, height: number, pixels: Uint8ClampedArray): HTMLCanvasElement {
	const output = canvas(width, height);
	const context = context2d(output);
	context.fillStyle = '#111827';
	context.fillRect(0, 0, width, height);
	context.putImageData(new ImageData(new Uint8ClampedArray(pixels), width, height), 0, 0);
	return output;
}

function canvasPng(canvas: HTMLCanvasElement): Promise<Blob> {
	return new Promise((resolve, reject) => {
		canvas.toBlob(
			(blob) => (blob ? resolve(blob) : reject(new Error('PNG encoding failed.'))),
			'image/png'
		);
	});
}

function createComparisonUrls(
	svg: string,
	actualRasterPng: Blob,
	referencePng: Blob,
	heatmapPng: Blob
): Pick<ComparisonPage, 'actualSvgUrl' | 'actualRasterUrl' | 'referenceUrl' | 'heatmapUrl'> {
	const urls: string[] = [];
	try {
		const actualSvgUrl = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
		urls.push(actualSvgUrl);
		const actualRasterUrl = URL.createObjectURL(actualRasterPng);
		urls.push(actualRasterUrl);
		const referenceUrl = URL.createObjectURL(referencePng);
		urls.push(referenceUrl);
		const heatmapUrl = URL.createObjectURL(heatmapPng);
		urls.push(heatmapUrl);
		return { actualSvgUrl, actualRasterUrl, referenceUrl, heatmapUrl };
	} catch (error) {
		for (const url of urls) URL.revokeObjectURL(url);
		throw error;
	}
}

export function disposeComparisonPages(pages: readonly ComparisonPage[]): void {
	for (const page of pages) {
		URL.revokeObjectURL(page.actualSvgUrl);
		URL.revokeObjectURL(page.actualRasterUrl);
		URL.revokeObjectURL(page.referenceUrl);
		URL.revokeObjectURL(page.heatmapUrl);
	}
}

export async function renderComparisons(
	client: ConverterClient,
	referencePdf: Uint8Array,
	visiblePageCount: number,
	signal: AbortSignal,
	onPage?: (completed: number, total: number) => void
): Promise<ComparisonSet> {
	throwIfAborted(signal);
	const loading = pdfjs.getDocument({ data: referencePdf });
	const pdf = (await loading.promise) as unknown as PdfDocumentProxy;
	const pages: ComparisonPage[] = [];
	try {
		if (pdf.numPages < visiblePageCount) {
			throw new Error(
				`Reference PDF has ${pdf.numPages} pages, but the renderer produced ${visiblePageCount}.`
			);
		}

		for (let pageIndex = 0; pageIndex < visiblePageCount; pageIndex += 1) {
			throwIfAborted(signal);
			const [svg, pdfPage] = await Promise.all([
				client.renderPage(pageIndex, 'light'),
				pdf.getPage(pageIndex + 1)
			]);
			const reference = await renderPdfPage(pdfPage, signal);
			const actual = await rasterizeSvg(
				svg,
				reference.canvas.width,
				reference.canvas.height,
				signal
			);
			const referenceContext = context2d(reference.canvas);
			const actualContext = context2d(actual.canvas);
			const comparison = comparePixels(
				actualContext.getImageData(0, 0, actual.canvas.width, actual.canvas.height),
				referenceContext.getImageData(
					0,
					0,
					reference.canvas.width,
					reference.canvas.height
				)
			);
			const heatmap = heatmapCanvas(
				reference.canvas.width,
				reference.canvas.height,
				comparison.heatmap
			);
			const [actualRasterPng, referencePng, heatmapPng] = await Promise.all([
				canvasPng(actual.canvas),
				canvasPng(reference.canvas),
				canvasPng(heatmap)
			]);
			throwIfAborted(signal);
			const metrics: PageVisualMetrics = {
				...comparison.metrics,
				...compareSourceDimensions(
					actual.sourceWidth,
					actual.sourceHeight,
					reference.sourceWidth,
					reference.sourceHeight
				)
			};
			const urls = createComparisonUrls(svg, actualRasterPng, referencePng, heatmapPng);
			pages.push({
				pageIndex,
				...urls,
				actualRasterPng,
				referencePng,
				heatmapPng,
				metrics
			});
			onPage?.(pageIndex + 1, visiblePageCount);
		}

		return { pages, referencePageCount: pdf.numPages };
	} catch (error) {
		disposeComparisonPages(pages);
		throw error;
	} finally {
		await pdf.destroy();
	}
}
