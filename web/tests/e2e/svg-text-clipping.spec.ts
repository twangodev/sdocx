import { expect, test } from '../fixtures/browser';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

test('Chromium span clips retain joined shaping and selectable text at glyph ownership boundaries', async ({ page, browserName }) => {
	test.skip(browserName !== 'chromium', 'Chromium is the current vector preview target.');
	const font = await readFile(resolve('../crates/sdocx/tests/assets/fonts/DejaVuSans.ttf'));
	const results = await page.evaluate(async font64 => {
		const create = <K extends keyof SVGElementTagNameMap>(tag: K): SVGElementTagNameMap[K] =>
			document.createElementNS('http://www.w3.org/2000/svg', tag);
		const cases = [
			{ name: 'ordinary', parts: ['MMMM', 'MMMM'] },
			{ name: 'Arabic', parts: ['سل', 'ام'] },
			{ name: 'ligature', parts: ['f', 'fi'] },
			{ name: 'combining', parts: ['a', '\u0301b'] },
			{ name: 'surrogate', parts: ['A😀', 'B'] }
		];
		const results = [];
		for (const example of cases) {
			const variants = [];
			for (const mode of ['text', 'spans', 'clip-full', 'clip-first', 'clip-all']) {
				const svg = create('svg');
				svg.setAttribute('width', '600');
				svg.setAttribute('height', '100');
				const defs = create('defs');
				const style = create('style');
				style.textContent = `@font-face { font-family: ClipReference; src: url(data:font/ttf;base64,${font64}) }`;
				defs.append(style);
				const clip = create('clipPath');
				clip.id = 'run-clip';
				clip.setAttribute('clipPathUnits', 'userSpaceOnUse');
				const rect = create('rect');
				for (const [key, value] of Object.entries({
					x: mode === 'clip-first' || mode === 'clip-all' ? '600' : '0',
					y: '0', width: '600', height: '100'
				})) rect.setAttribute(key, value);
				clip.append(rect);
				defs.append(clip);
				svg.append(defs);
				const text = create('text');
				for (const [key, value] of Object.entries({ x: '150', y: '70', 'font-size': '48', 'font-family': 'ClipReference' })) {
					text.setAttribute(key, value);
				}
				if (mode === 'text') text.textContent = example.parts.join('');
				else for (const [index, part] of example.parts.entries()) {
					const span = create('tspan');
					span.textContent = part;
					if (index === 0 && (mode === 'clip-full' || mode === 'clip-first')) span.setAttribute('clip-path', 'url(#run-clip)');
					text.append(span);
				}
				if (mode === 'clip-all') text.setAttribute('clip-path', 'url(#run-clip)');
				svg.append(text);
				document.body.append(svg);
				const url = URL.createObjectURL(new Blob([new XMLSerializer().serializeToString(svg)], { type: 'image/svg+xml' }));
				try {
					const image = new Image();
					image.src = url;
					await image.decode();
					const canvas = document.createElement('canvas');
					canvas.width = 600;
					canvas.height = 100;
					const context = canvas.getContext('2d')!;
					context.drawImage(image, 0, 0);
					const pixels = context.getImageData(0, 0, 600, 100).data;
					let ink = 0;
					for (let index = 3; index < pixels.length; index += 4) if (pixels[index]) ink++;
					const selection = getSelection()!;
					const range = document.createRange();
					range.selectNodeContents(text);
					selection.removeAllRanges();
					selection.addRange(range);
					variants.push({ mode, ink, image: canvas.toDataURL(), source: selection.toString() });
				} finally {
					URL.revokeObjectURL(url);
					svg.remove();
				}
			}
			results.push({ ...example, variants });
		}
		return results;
	}, font.toString('base64'));
	for (const example of results) {
		const [text, spans, full, first, all] = example.variants;
		expect(text.ink, example.name).toBeGreaterThan(0);
		expect(spans.image, example.name).toBe(text.image);
		expect(full.image, example.name).toBe(text.image);
		expect(first.ink, example.name).toBeLessThan(text.ink);
		expect(all.ink, example.name).toBe(0);
		for (const variant of example.variants) expect(variant.source, `${example.name}/${variant.mode}`).toBe(example.parts.join(''));
		if (example.name === 'ligature') expect(first.ink).toBe(0);
		else expect(first.ink, example.name).toBeGreaterThan(0);
	}
});
