import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { expect, test } from '../fixtures/browser';

const fixture = resolve('../crates/sdocx/tests/fixtures/inspection_integers.sdocx');
const timestamps = [
	42, 9_007_199_254_740_991, -9_007_199_254_740_991,
	'9007199254740992', '-9007199254740992',
	'9223372036854775807', '-9223372036854775808'
] as const;

test('WASM inspection exports exact stroke and shape timestamps as JSON', async ({ page }) => {
	const bytes = await readFile(fixture);
	expect(bytes.length).toBe(380);
	expect(createHash('sha256').update(bytes).digest('hex')).toBe(
		'e1806921b031776910ce7185738c3e69e49c3b2cd3f249d3e3f8b6a592591f2a'
	);
	await page.route('https://rybbit.twango.dev/api/script.js', route => route.fulfill({ body: '' }));
	await page.goto('/');
	const results = await page.evaluate(async bytes => {
		const module = await import(`${location.origin}/wasm/sdocx_wasm.js`);
		await module.default();
		const session = new module.DocumentSession(new Uint8Array(bytes));
		try {
			type Metadata = { uuid: string; modified_time_raw: number | string };
			type ObjectSource = { content: {
				Stroke?: { rendering: { metadata: Metadata }; timestamps: number[] };
				Element?: { Shape: { metadata: Metadata } };
			} };
			const summarize = (objects: ObjectSource[]) => objects.map(object => {
				const stroke = object.content.Stroke;
				const metadata = stroke?.rendering.metadata ?? object.content.Element?.Shape.metadata;
				if (!metadata) throw new Error('Missing retained object metadata');
				return { timestamp: metadata.modified_time_raw, uuid: metadata.uuid, points: stroke?.timestamps ?? null };
			});
			const inspection = session.inspection();
			const json = JSON.parse(JSON.stringify(inspection));
			return [
				summarize(inspection.document.pages[0].objects),
				summarize(inspection.layout.pages[0].page.objects),
				summarize(json.document.pages[0].objects),
				summarize(json.layout.pages[0].page.objects)
			];
		} finally {
			session.dispose();
			session.free();
		}
	}, [...bytes]);
	const expected = timestamps.flatMap(timestamp => [
		{ timestamp, uuid: 'stamp', points: [17] },
		{ timestamp, uuid: 'stamp', points: null }
	]);
	for (const values of results) expect(values).toEqual(expected);
});
