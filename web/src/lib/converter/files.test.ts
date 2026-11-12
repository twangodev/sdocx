import { strFromU8, unzipSync } from 'fflate';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createExportManifest, createZip, downloadBlob, pageFilename, sanitizeStem, textBytes } from './files';

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

	it('rejects when producing an archive entry fails', async () => {
		async function* entries() {
			yield { name: 'page-001.svg', bytes: textBytes('<svg/>') };
			throw new Error('render failed');
		}

		await expect(createZip(entries())).rejects.toThrow('render failed');
	});
});

describe('download URL ownership', () => {
	afterEach(() => {
		vi.useRealTimers();
		vi.restoreAllMocks();
		vi.unstubAllGlobals();
	});

	function harness() {
		vi.useFakeTimers();
		const anchor = { href: '', download: '', click: vi.fn() };
		const createAnchor = vi.fn(() => anchor);
		vi.stubGlobal('document', { createElement: createAnchor });
		const createUrl = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:download');
		const revokeUrl = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
		return { anchor, createAnchor, createUrl, revokeUrl };
	}

	it('starts a download before revoking its URL on the next timer turn', () => {
		const { anchor, createAnchor, createUrl, revokeUrl } = harness();
		const blob = new Blob(['download']);
		downloadBlob(blob, 'note.svg');
		expect(createUrl).toHaveBeenCalledExactlyOnceWith(blob);
		expect(createAnchor).toHaveBeenCalledExactlyOnceWith('a');
		expect(anchor.href).toBe('blob:download');
		expect(anchor.download).toBe('note.svg');
		expect(anchor.click).toHaveBeenCalledOnce();
		expect(revokeUrl).not.toHaveBeenCalled();
		vi.runOnlyPendingTimers();
		expect(revokeUrl).toHaveBeenCalledExactlyOnceWith('blob:download');
	});

	it.each(['setup', 'click'] as const)('releases the URL after %s fails without replacing the original error', (failure) => {
		const { anchor, createAnchor, revokeUrl } = harness();
		const expected = new Error(`${failure} failed`);
		const fail = () => { throw expected; };
		if (failure === 'setup') createAnchor.mockImplementationOnce(fail);
		else anchor.click.mockImplementationOnce(fail);
		let caught: unknown;
		try {
			downloadBlob(new Blob(['download']), 'note.svg');
		} catch (error) {
			caught = error;
		}
		expect(caught).toBe(expected);
		expect(revokeUrl).not.toHaveBeenCalled();
		vi.runOnlyPendingTimers();
		expect(revokeUrl).toHaveBeenCalledExactlyOnceWith('blob:download');
	});
});
