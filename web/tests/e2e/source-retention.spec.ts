import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { expect, test } from '../fixtures/browser';

const fixture = resolve('../crates/sdocx/tests/fixtures/source_retention.sdocx');
const pdf = Buffer.from('%PDF-1.7\0\xff\x80retained paper source\n', 'latin1');
const gif = Buffer.from('GIF89a\0\xffretained animation source', 'latin1');
const png = Buffer.from([
	137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
	8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207,
	192, 240, 31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130
]);

function sourcePayloadPaths(value: unknown, path = ''): string[] {
	if (!value || typeof value !== 'object') return [];
	return Object.entries(value).flatMap(([key, child]) => {
		const childPath = path ? `${path}.${key}` : key;
		return ['data', 'bytes', 'trailing_data'].includes(key)
			? [childPath]
			: sourcePayloadPaths(child, childPath);
	});
}

test('WASM inspection retains PDF paper identities and bounded archive summaries', async ({ page }) => {
	const note = await readFile(fixture);
	expect(note.length).toBeLessThanOrEqual(5 * 1024);
	expect(createHash('sha256').update(note).digest('hex')).toBe(
		'5911cd25dad544d208ee7385ea85442209800bda239d67ebb1246b4d7765fc94'
	);
	await page.route('https://rybbit.twango.dev/api/script.js', route => route.fulfill({ body: '' }));
	await page.goto('/');
	const result = await page.evaluate(async bytes => {
		const module = await import(`${location.origin}/wasm/sdocx_wasm.js`);
		await module.default();
		const session = new module.DocumentSession(new Uint8Array(bytes));
		try {
			const inspection = JSON.parse(JSON.stringify(session.inspection(), (_, value) => value === undefined ? null : value));
			const request = (value: object) => JSON.parse(session.debug(JSON.stringify(value)));
			const index = request({ kind: 'index' });
			const entryIndex = (name: string): number => {
				const entry = index.entries.find((value: { name: string; index: number }) => value.name === name);
				if (!entry) throw new Error(`Missing retained source ${name}`);
				return entry.index;
			};
			const document = request({ kind: 'document' });
			const storedPage = request({ kind: 'page', page: 0 });
			const mediaManifest = request({ kind: 'entry', entry: entryIndex('media/mediaInfo.dat') });
			const pageManifest = request({ kind: 'entry', entry: entryIndex('pageIdInfo.dat') });
			const mediaSource = request({ kind: 'bytes', entry: entryIndex('media/mediaInfo.dat'), offset: 0, length: 4096 });
			const pageSource = request({ kind: 'bytes', entry: entryIndex('pageIdInfo.dat'), offset: 0, length: 4096 });
			const opaque = ['media/9@paper.pdf', 'media/animation.gif'].map(name => ({
				name,
				entry: request({ kind: 'entry', entry: entryIndex(name) }),
				source: request({ kind: 'bytes', entry: entryIndex(name), offset: 0, length: 4096 })
			}));
			const svg = new DOMParser().parseFromString(session.render_svg(0, 'light'), 'image/svg+xml');
			session.dispose();
			const disposed = [
				() => session.inspection(),
				() => session.debug(JSON.stringify({ kind: 'index' })),
				() => session.render_svg(0, 'light')
			].map(operation => {
				try { operation(); return ''; }
				catch (error) { return error instanceof Error ? error.message : String(error); }
			});
			return {
				metadata: inspection.document.metadata,
				background: inspection.document.pages[0].background,
				pageManifest: inspection.page_manifest,
				document, storedPage, mediaManifest, decodedPageManifest: pageManifest, mediaSource, pageSource, opaque,
				renderedImageCount: svg.querySelectorAll('image, foreignObject').length,
				disposed, disposedPageCount: session.page_count
			};
		} finally { session.free(); }
	}, [...note]);

	expect(result.background).toMatchObject({
		template_type: 11,
		pdf_paper: [
			{ media_id: 42, page_index: 99, rectangle: { Integer: [0, 0, 1080, 700] } },
			{ media_id: 42, page_index: -1, rectangle: { Integer: [0, 700, 1080, 1527] } },
			{ media_id: -1, page_index: 7, rectangle: { Integer: [-5, 3, 4, 9] } }
		]
	});
	expect(result.storedPage.background).toEqual(result.background);
	expect(result.metadata.page_ids).toEqual(['page']);
	const manifest = result.metadata.media_manifest;
	expect(manifest.format_version).toBe(5500);
	expect(manifest.trailing_byte_length).toBe(1024);
	expect(manifest.entries).toEqual([
		{ bind_id: 42, file_name: '9@paper.pdf', is_attached: true },
		{ bind_id: 3, file_name: 'animation.gif', is_attached: false },
		{ bind_id: 100, file_name: '2@image.png', is_attached: true }
	].map(entry => ({
		...entry, sha256: 'a'.repeat(64), reference_count: 3,
		modified_time_raw: '9223372036854775807', trailing_byte_length: 512
	})));
	expect(result.mediaManifest).toEqual(manifest);
	expect(result.document.metadata.media_manifest).toEqual(manifest);
	expect(result.pageManifest).toEqual({
		integrity_header: Array(32).fill(0x11),
		entries: [{ page_id: 'page', integrity_hash: Array(32).fill(0x22) }],
		trailing_byte_length: 1024
	});
	expect(result.decodedPageManifest).toEqual(result.pageManifest);
	expect(result.document.manifest).toEqual(result.pageManifest);
	expect(createHash('sha256').update(Buffer.from(result.mediaSource.bytes)).digest('hex')).toBe(
		'ca7f289fa5897774fd01f1adf57d2286bad11391042ab98058c205707bb25443'
	);
	expect(createHash('sha256').update(Buffer.from(result.pageSource.bytes)).digest('hex')).toBe(
		'7cf194eda51fae7a18c58a12fdb04a91b2f66067d62f872e22491758c21b3a52'
	);
	expect(result.mediaSource.bytes.slice(-manifest.trailing_byte_length)).toEqual(Array(1024).fill(0x82));
	expect(result.pageSource.bytes.slice(-result.pageManifest.trailing_byte_length)).toEqual(Array(1024).fill(0x83));

	const resources = result.metadata.archive_resources;
	expect(resources).toHaveLength(4);
	expect(result.document.metadata.archive_resources).toEqual(resources);
	for (const [index, resource] of resources.entries()) {
		expect(resource.resource_index).toBe(index);
		expect(resource.data_available).toBe(true);
	}
	for (const [name, bytes] of [['media/9@paper.pdf', pdf], ['media/animation.gif', gif]] as const) {
		const resource = resources.find((value: { name: string }) => value.name === name);
		expect(resource).toMatchObject({ kind: 'opaque', media_index: null, byte_length: bytes.length });
		const drilldown = result.opaque.find((value: { name: string }) => value.name === name)!;
		expect(drilldown.entry).toMatchObject({ name, byteLength: bytes.length });
		expect(drilldown.source).toEqual({ offset: 0, total: bytes.length, bytes: [...bytes] });
	}
	expect(resources.find((value: { name: string }) => value.name === 'media/9@paper.pdf').archive_id).toBe(9);
	expect(resources.find((value: { name: string }) => value.name === 'media/mediaInfo.dat'))
		.toMatchObject({ kind: 'opaque', byte_length: 2895 });
	expect(resources.find((value: { name: string }) => value.name === 'media/2@image.png'))
		.toMatchObject({ kind: 'media_asset', media_index: 0, archive_id: 2, byte_length: png.length });
	const imageDigest = createHash('sha256').update(png).digest('hex');
	expect(result.metadata.media_assets).toEqual([{
		name: 'media/2@image.png', archive_id: 2, mime_type: 'image/png', byte_length: png.length, sha256: imageDigest
	}]);
	expect(imageDigest).not.toBe(manifest.entries[2].sha256);
	expect(result.renderedImageCount).toBe(0);
	for (const summary of [result.metadata, result.pageManifest, result.document.metadata, result.document.manifest,
		result.mediaManifest, result.decodedPageManifest]) {
		expect(sourcePayloadPaths(summary)).toEqual([]);
	}
	expect(JSON.stringify(result.metadata).length).toBeLessThan(4096);
	expect(result.disposedPageCount).toBe(0);
	expect(result.disposed).toHaveLength(3);
	for (const message of result.disposed) expect(message).toMatch(/disposed/);
});
