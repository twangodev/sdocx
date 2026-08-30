import { strFromU8, unzipSync } from 'fflate';
import { describe, expect, it } from 'vitest';
import { createExportManifest, createZip, pageFilename, sanitizeStem, textBytes } from './files';

describe('converter filenames', () => {
	it('turns untrusted source names into portable download names', () => {
		expect(sanitizeStem('../../My résumé / final.sdocx')).toBe('My-resume-final');
		expect(sanitizeStem('...sdocx')).toBe('document');
	});

	it('pads page numbers for stable lexical ordering', () => {
		expect(pageFilename('meeting', 0, 'svg')).toBe('meeting-page-001.svg');
		expect(pageFilename('meeting', 11, 'png')).toBe('meeting-page-012.png');
	});

	it('produces a deterministic export manifest when given a timestamp', () => {
		expect(
			createExportManifest('meeting.sdocx', 3, 'dark', 2, new Date('2026-08-30T00:00:00Z'))
		).toEqual({
			format: 1,
			generator: 'sdocx.twango.dev',
			source: 'meeting.sdocx',
			pageCount: 3,
			colorMode: 'dark',
			pngScale: 2,
			createdAt: '2026-08-30T00:00:00.000Z'
		});
	});

	it('streams entries into a valid archive', async () => {
		async function* entries() {
			yield { name: 'page-001.svg', bytes: textBytes('<svg/>') };
			yield { name: 'manifest.json', bytes: textBytes('{"pages":1}') };
		}

		const archive = await createZip(entries());
		const files = unzipSync(new Uint8Array(await archive.arrayBuffer()));
		expect(strFromU8(files['page-001.svg'])).toBe('<svg/>');
		expect(strFromU8(files['manifest.json'])).toBe('{"pages":1}');
	});
});
