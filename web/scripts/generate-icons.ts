import { chromium } from '@playwright/test';
import { writeFile } from 'node:fs/promises';

const glyph = `<path d="M24 8.5c-2-1.4-4.4-2.2-7.2-2.2-4.8 0-8 2.2-8 5.7 0 3 2.1 4.6 6.7 5.5 3.3.6 4.5 1.3 4.5 2.6 0 1.5-1.6 2.4-4.2 2.4-2.7 0-5.3-.9-7.4-2.4" fill="none" stroke="#e8e5df" stroke-linecap="round" stroke-width="3.5"/>
	<rect x="23" y="22" width="3.5" height="3.5" rx=".7" fill="#e8e5df"/>`;

function mark(scale: number, radius = 0): string {
	return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32">
		<rect width="32" height="32" rx="${radius}" fill="#1a1916"/>
		<g transform="translate(16 16) scale(${scale}) translate(-16 -16)">${glyph}</g>
	</svg>`;
}

function asIco(images: Array<{ size: number; png: Buffer }>): Buffer {
	const directorySize = 6 + images.length * 16;
	const header = Buffer.alloc(directorySize);
	header.writeUInt16LE(1, 2);
	header.writeUInt16LE(images.length, 4);

	let imageOffset = directorySize;
	for (const [index, image] of images.entries()) {
		const entryOffset = 6 + index * 16;
		header.writeUInt8(image.size, entryOffset);
		header.writeUInt8(image.size, entryOffset + 1);
		header.writeUInt16LE(1, entryOffset + 4);
		header.writeUInt16LE(32, entryOffset + 6);
		header.writeUInt32LE(image.png.length, entryOffset + 8);
		header.writeUInt32LE(imageOffset, entryOffset + 12);
		imageOffset += image.png.length;
	}

	return Buffer.concat([header, ...images.map(({ png }) => png)]);
}

const browser = await chromium.launch({ headless: true });

async function render(svg: string, size: number): Promise<Buffer> {
	const page = await browser.newPage({ viewport: { width: size, height: size } });
	await page.setContent(`<style>html,body,svg{width:100%;height:100%;margin:0;display:block}</style>${svg}`);
	const png = await page.screenshot({ type: 'png' });
	await page.close();
	return png;
}

try {
	const faviconSizes = [16, 32, 48];
	const faviconImages: Array<{ size: number; png: Buffer }> = [];
	for (const size of faviconSizes) {
		faviconImages.push({ size, png: await render(mark(0.76, 6), size) });
	}
	await Promise.all([
		writeFile('static/favicon.ico', asIco(faviconImages)),
		writeFile('static/apple-touch-icon.png', await render(mark(0.76), 180)),
		writeFile('static/icon-192.png', await render(mark(0.76, 6), 192)),
		writeFile('static/icon-512.png', await render(mark(0.76, 6), 512)),
		writeFile('static/icon-maskable.png', await render(mark(0.6), 512))
	]);
} finally {
	await browser.close();
}

console.log('icons written to static/');
