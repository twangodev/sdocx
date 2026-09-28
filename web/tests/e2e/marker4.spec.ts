import { test, expect } from '../fixtures/browser';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import type { Replay } from '../../src/lib/debugger/model';

test('V7 highlighters retain native width and opacity in WASM export and replay', async ({ page }) => {
	const fixture = resolve('../tmp/stroke-conformance/quiz.sdocx');
	test.skip(!existsSync(fixture), 'Local V7 fixture is absent');
	await page.route('**/__marker4.sdocx', route => route.fulfill({ body: readFileSync(fixture) }));
	await page.goto('/');
	const result = await page.evaluate(async () => {
		const moduleUrl = '/wasm/sdocx_wasm.js';
		const wasm = await import(moduleUrl) as {
			default: () => Promise<unknown>;
			DocumentSession: new (bytes: Uint8Array) => {
				debug(request: string): string;
				render_svg(page: number, color: string): string;
				free(): void;
			};
		};
		await wasm.default();
		const bytes = new Uint8Array(await (await fetch('/__marker4.sdocx')).arrayBuffer());
		const session = new wasm.DocumentSession(bytes);
		try {
			const replay: Replay = JSON.parse(session.debug(JSON.stringify({ kind: 'replay', page: 0 })));
			const exported = new DOMParser().parseFromString(session.render_svg(0, 'light'), 'image/svg+xml');
			const { svg } = JSON.parse(session.debug(JSON.stringify({ kind: 'replay-svg', page: 0, colorMode: 'light' }))) as { svg: string };
			const annotated = new DOMParser().parseFromString(svg, 'image/svg+xml');
			const markers = replay.strokes.flatMap((stroke, index) => stroke.geometry.profile === 'Marker4' ? [{ ...stroke, index }] : []);
			const paths = [...exported.querySelectorAll('g[style="mix-blend-mode:darken"] > path')];
			const samePaths = markers.every((stroke, i) => {
				const path = annotated.querySelector(`[data-replay-stroke="${stroke.index}"] path`);
				return path?.getAttribute('d') === paths[i]?.getAttribute('d') &&
					path?.getAttribute('fill-opacity') === paths[i]?.getAttribute('fill-opacity') &&
					path?.getAttribute('data-replay-lengths')?.split(',').length === stroke.geometry.points?.length;
			});
			const isolated = exported.documentElement.cloneNode(false) as SVGSVGElement;
			isolated.setAttribute('width', '1812');
			isolated.setAttribute('height', '3843');
			isolated.setAttribute('viewBox', '0 0 1812 3843');
			isolated.append(exported.querySelector('rect')!.cloneNode(true), paths[0].cloneNode(true));
			const url = URL.createObjectURL(new Blob([new XMLSerializer().serializeToString(isolated)], { type: 'image/svg+xml' }));
			try {
				const image = new Image(); image.src = url; await image.decode();
				const canvas = document.createElement('canvas'); canvas.width = 1812; canvas.height = 3843;
				const context = canvas.getContext('2d')!; context.drawImage(image, 0, 0);
				return {
					count: markers.length,
					stamps: markers.reduce((sum, stroke) => sum + (stroke.geometry.points?.length ?? 0), 0),
					height: markers[0].geometry.rect_stamp?.height,
					opacity: markers[0].geometry.opacity,
					samePaths,
					pixel: [...context.getImageData(200, 1790, 1, 1).data],
					images: isolated.querySelectorAll('image, filter').length
				};
			} finally { URL.revokeObjectURL(url); }
		} finally { session.free(); }
	});
	expect(result.count).toBe(6);
	expect(result.stamps).toBe(18660);
	expect(result.height).toBeCloseTo(36.140083, 5);
	expect(result.opacity).toBeCloseTo(115 / 255, 6);
	expect(result.samePaths).toBe(true);
	expect(result.images).toBe(0);
	for (const [index, expected] of [161, 206, 253, 255].entries()) {
		expect(Math.abs(result.pixel[index] - expected)).toBeLessThanOrEqual(1);
	}
});
