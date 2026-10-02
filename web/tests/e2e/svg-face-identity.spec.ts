import { zipSync } from 'fflate';
import { expect, test } from '../fixtures/browser';
import { join, zero, u16, u32, f32, f64, frame } from '../fixtures/wdoc';

function mixedFaceNote(): Buffer {
	const source = 'WW';
	const fontName = (start: number, end: number, name: string) => {
		const family = Buffer.from(name, 'utf8');
		const payload = join(zero(8), u16(family.length + 1), family, zero(1));
		return join(u16(16 + payload.length), ...[4, start, end, 1].map(u32), payload);
	};
	const spans = [
		join(u16(20), ...[3, 0, 2, 1].map(u32), f32(20)),
		join(u16(20), ...[1, 0, 2, 1, 0xff000000].map(u32)),
		fontName(1, 2, 'Roboto-Bold'),
		join(u16(18), ...[5, 0, 2, 1].map(u32), u16(1))
	];
	const common = join(
		u32(source.length), Buffer.from(source, 'utf16le'), u32(spans.length),
		...spans, u32(0), zero(16), zero(11)
	);
	const base = frame(
		0, 1,
		join(u32(5500), u16(2), Buffer.from('tx'), zero(8), ...[10, 20, 310, 220].map(f64), zero(5)),
		f32(0)
	);
	const payload = join(base, frame(6, 0), frame(7, 1, zero(0), join(u32(common.length), common)), frame(2, 0));
	const object = join(Buffer.from([2]), u16(0), u32(payload.length + 32), payload, zero(32));
	const header = join(
		zero(8), Buffer.from([1, 0, 5]), zero(5), ...[0, 400, 400, 0, 0].map(u32),
		u16(4), Buffer.from('face', 'utf16le'), zero(8), u32(5500), u32(4000)
	);
	header.writeUInt32LE(header.length, 0);
	header.writeUInt32LE(header.length, 4);
	const layer = join(u32(20), zero(4), Buffer.from([2, 2, 0, 3, 0, 0, 0]), zero(5), u32(1), object, zero(32));
	return Buffer.from(zipSync({
		'face.page': join(header, u16(1), u16(0), layer, zero(32), Buffer.from('Page for SAMSUNG S-Pen SDK'))
	}));
}

test('SVG synthetic bold retains the selected physical face beside an embedded bold face', async ({ page, browserName }) => {
	test.skip(browserName !== 'chromium', 'Chromium preview and vector exports are the immediate target.');
	await page.route('https://rybbit.twango.dev/api/script.js', route => route.fulfill({ body: '' }));
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Your notes, in one place', exact: true })).toBeVisible();
	const svg = await page.evaluate(async note => {
		const module = await import(`${location.origin}/wasm/sdocx_wasm.js`);
		await module.default();
		const session = new module.DocumentSession(new Uint8Array(note));
		try {
			return session.render_svg(0, 'light') as string;
		} finally {
			session.free();
		}
	}, [...mixedFaceNote()]);
	await page.route('**/physical-face.svg', route => route.fulfill({ contentType: 'image/svg+xml', body: svg }));
	await page.goto('/physical-face.svg');
	const result = await page.evaluate(async () => {
		const preview = document.documentElement;
		const spans = [...preview.querySelectorAll('tspan')].filter(span => span.textContent === 'W');
		spans.forEach((span, index) => {
			span.id = `physical-face-${index}`;
		});
		await Promise.all(spans.map(span => document.fonts.load(getComputedStyle(span).font, 'W')));
		await document.fonts.ready;
		return {
			source: preview.querySelector('text')?.textContent,
			families: spans.map(span => getComputedStyle(span).fontFamily),
			weights: spans.map(span => getComputedStyle(span).fontWeight),
			images: preview.querySelectorAll('image, foreignObject').length,
			fontFaces: [...preview.querySelectorAll('style')]
				.map(style => style.textContent).join('').match(/@font-face/g)?.length
		};
	});
	expect(result.source).toBe('WW');
	expect(result.families).toHaveLength(2);
	expect(result.families[0]).not.toBe(result.families[1]);
	expect(result.weights).toEqual(['700', '700']);
	expect(result.fontFaces).toBe(2);
	expect(result.images).toBe(0);
	const client = await page.context().newCDPSession(page);
	try {
		await client.send('DOM.enable');
		await client.send('CSS.enable');
		const { root } = await client.send('DOM.getDocument');
		for (const [index, expected] of ['Roboto-Regular', 'Roboto-Bold'].entries()) {
			const { nodeId } = await client.send('DOM.querySelector', {
				nodeId: root.nodeId, selector: `#physical-face-${index}`
			});
			const { fonts } = await client.send('CSS.getPlatformFontsForNode', { nodeId });
			expect(fonts.map(font => ({
				name: font.postScriptName, custom: font.isCustomFont, glyphs: font.glyphCount
			}))).toEqual([{ name: expected, custom: true, glyphs: 1 }]);
		}
	} finally {
		await client.detach();
	}
});
