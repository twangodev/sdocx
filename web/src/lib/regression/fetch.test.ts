import { describe, expect, it } from 'vitest';
import {
	fetchVerifiedAsset,
	readVerifiedFileAsset,
	sha256Hex,
	transferableBuffer
} from './fetch';

const ABC_SHA256 = 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad';

describe('verified regression asset fetches', () => {
	it('hashes bytes through Web Crypto', async () => {
		expect(await sha256Hex(new TextEncoder().encode('abc'))).toBe(ABC_SHA256);
	});

	it('streams bytes, reports progress, and verifies the expected digest', async () => {
		const progress: number[] = [];
		const fetcher = async () =>
			new Response(
				new ReadableStream<Uint8Array>({
					start(controller) {
						controller.enqueue(new TextEncoder().encode('a'));
						controller.enqueue(new TextEncoder().encode('bc'));
						controller.close();
					}
				}),
				{ headers: { 'content-length': '3', 'content-type': 'application/octet-stream' } }
			);
		const asset = await fetchVerifiedAsset('https://example.invalid/fixture', ABC_SHA256, {
			fetcher: fetcher as typeof fetch,
			onProgress: ({ receivedBytes }) => progress.push(receivedBytes)
		});

		expect(new TextDecoder().decode(asset.bytes)).toBe('abc');
		expect(asset.sha256).toBe(ABC_SHA256);
		expect(asset.contentType).toBe('application/octet-stream');
		expect(progress).toEqual([1, 3]);
	});

	it('rejects a digest mismatch and returns an owned transferable buffer', async () => {
		await expect(
			fetchVerifiedAsset('https://example.invalid/fixture', '0'.repeat(64), {
				fetcher: (async () => new Response('abc')) as typeof fetch
			})
		).rejects.toThrow(/SHA-256 mismatch/);

		const source = Uint8Array.from([9, 1, 2, 8]).subarray(1, 3);
		const output = transferableBuffer(source);
		expect(Array.from(new Uint8Array(output))).toEqual([1, 2]);
	});

	it('streams and verifies a user-selected local fixture file', async () => {
		const file = new File(['abc'], 'fixture.sdocx', { type: 'application/zip' });
		const progress: number[] = [];
		const asset = await readVerifiedFileAsset(file, ABC_SHA256, {
			onProgress: ({ receivedBytes }) => progress.push(receivedBytes)
		});

		expect(new TextDecoder().decode(asset.bytes)).toBe('abc');
		expect(asset.contentType).toBe('application/zip');
		expect(progress.at(-1)).toBe(3);
	});

	it('rejects oversized declared, streamed, and local fixture assets', async () => {
		await expect(
			fetchVerifiedAsset('https://example.invalid/fixture', ABC_SHA256, {
				maxBytes: 2,
				fetcher: (async () =>
					new Response('abc', { headers: { 'content-length': '3' } })) as typeof fetch
			})
		).rejects.toThrow(/2 bytes/);

		await expect(
			fetchVerifiedAsset('https://example.invalid/fixture', ABC_SHA256, {
				maxBytes: 2,
				fetcher: (async () =>
					new Response(
						new ReadableStream<Uint8Array>({
							start(controller) {
								controller.enqueue(new TextEncoder().encode('abc'));
								controller.close();
							}
						})
					)) as typeof fetch
			})
		).rejects.toThrow(/2 bytes/);

		await expect(
			readVerifiedFileAsset(new File(['abc'], 'fixture.sdocx'), ABC_SHA256, { maxBytes: 2 })
		).rejects.toThrow(/2 bytes/);
	});

	it('reuses an already-owned buffer for worker transfer', () => {
		const bytes = new Uint8Array([1, 2, 3]);
		expect(transferableBuffer(bytes)).toBe(bytes.buffer);
	});

	it('refuses a response that cannot enforce the streamed byte limit', async () => {
		const response = {
			ok: true,
			status: 200,
			headers: new Headers(),
			body: null
		} as Response;
		await expect(
			fetchVerifiedAsset('https://example.invalid/fixture', ABC_SHA256, {
				fetcher: (async () => response) as typeof fetch
			})
		).rejects.toThrow(/bounded stream/);
	});
});
