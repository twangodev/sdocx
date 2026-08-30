import { Zip, ZipPassThrough, strToU8 } from 'fflate';

export function sanitizeStem(filename: string): string {
	const withoutExtension = filename.replace(/\.sdocx$/i, '');
	const safe = withoutExtension
		.normalize('NFKD')
		.replace(/[\u0300-\u036f]/g, '')
		.replace(/[^a-zA-Z0-9._-]+/g, '-')
		.replace(/^[._-]+|[._-]+$/g, '')
		.slice(0, 80);
	return safe || 'document';
}

export function pageFilename(stem: string, pageIndex: number, extension: string): string {
	return `${stem}-page-${String(pageIndex + 1).padStart(3, '0')}.${extension}`;
}

export function downloadBlob(blob: Blob, filename: string): void {
	const url = URL.createObjectURL(blob);
	const anchor = document.createElement('a');
	anchor.href = url;
	anchor.download = filename;
	anchor.click();
	setTimeout(() => URL.revokeObjectURL(url), 0);
}

async function imageFromBlob(blob: Blob): Promise<HTMLImageElement> {
	const url = URL.createObjectURL(blob);
	try {
		const image = new Image();
		await new Promise<void>((resolve, reject) => {
			image.onload = () => resolve();
			image.onerror = () => reject(new Error('The SVG page could not be rasterized.'));
			image.src = url;
		});
		return image;
	} finally {
		URL.revokeObjectURL(url);
	}
}

export async function svgToPng(svg: string, scale: 1 | 2): Promise<Blob> {
	const svgBlob = new Blob([svg], { type: 'image/svg+xml' });
	let source: ImageBitmap | HTMLImageElement;
	try {
		source = await createImageBitmap(svgBlob);
	} catch {
		source = await imageFromBlob(svgBlob);
	}

	const width = Math.max(1, Math.round(source.width * scale));
	const height = Math.max(1, Math.round(source.height * scale));

	if (typeof OffscreenCanvas !== 'undefined') {
		const canvas = new OffscreenCanvas(width, height);
		const context = canvas.getContext('2d');
		if (!context) throw new Error('Canvas rendering is unavailable in this browser.');
		context.drawImage(source, 0, 0, width, height);
		if ('close' in source && typeof source.close === 'function') source.close();
		return canvas.convertToBlob({ type: 'image/png' });
	}

	const canvas = document.createElement('canvas');
	canvas.width = width;
	canvas.height = height;
	const context = canvas.getContext('2d');
	if (!context) throw new Error('Canvas rendering is unavailable in this browser.');
	context.drawImage(source, 0, 0, width, height);
	if ('close' in source && typeof source.close === 'function') source.close();
	return new Promise<Blob>((resolve, reject) => {
		canvas.toBlob(
			(blob) => (blob ? resolve(blob) : reject(new Error('PNG encoding failed.'))),
			'image/png'
		);
	});
}

export interface ExportManifest {
	format: 1;
	generator: 'sdocx.twango.dev';
	source: string;
	pageCount: number;
	colorMode: string;
	pngScale: number;
	createdAt: string;
}

export function createExportManifest(
	source: string,
	pageCount: number,
	colorMode: string,
	pngScale: number,
	createdAt = new Date()
): ExportManifest {
	return {
		format: 1,
		generator: 'sdocx.twango.dev',
		source,
		pageCount,
		colorMode,
		pngScale,
		createdAt: createdAt.toISOString()
	};
}

export async function createZip(
	entries: AsyncIterable<{ name: string; bytes: Uint8Array }>
): Promise<Blob> {
	return new Promise<Blob>(async (resolve, reject) => {
		const chunks: ArrayBuffer[] = [];
		const zip = new Zip((error, data, final) => {
			if (error) {
				reject(error);
				return;
			}
			const chunk = new ArrayBuffer(data.byteLength);
			new Uint8Array(chunk).set(data);
			chunks.push(chunk);
			if (final) resolve(new Blob(chunks, { type: 'application/zip' }));
		});

		try {
			for await (const entry of entries) {
				const file = new ZipPassThrough(entry.name);
				zip.add(file);
				file.push(entry.bytes, true);
			}
			zip.end();
		} catch (error) {
			zip.terminate();
			reject(error);
		}
	});
}

export function textBytes(value: string): Uint8Array {
	return strToU8(value);
}
