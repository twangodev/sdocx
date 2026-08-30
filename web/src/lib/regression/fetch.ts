import { MAX_INPUT_BYTES } from '../converter/protocol';

export interface DownloadProgress {
	receivedBytes: number;
	totalBytes?: number;
}

export interface VerifiedAsset {
	bytes: Uint8Array<ArrayBuffer>;
	sha256: string;
	contentType?: string;
}

export interface FetchVerifiedOptions {
	signal?: AbortSignal;
	onProgress?: (progress: DownloadProgress) => void;
	fetcher?: typeof fetch;
	maxBytes?: number;
}

export type VerifyAssetOptions = Omit<FetchVerifiedOptions, 'fetcher'>;

function abortError(): DOMException {
	return new DOMException('The regression run was cancelled.', 'AbortError');
}

function ensureNotAborted(signal?: AbortSignal): void {
	if (signal?.aborted) throw signal.reason ?? abortError();
}

function concatenate(chunks: Uint8Array[], size: number): Uint8Array<ArrayBuffer> {
	const bytes = new Uint8Array(size);
	let offset = 0;
	for (const chunk of chunks) {
		bytes.set(chunk, offset);
		offset += chunk.byteLength;
	}
	return bytes;
}

async function readVerifiedStream(
	stream: ReadableStream<Uint8Array>,
	expectedSha256: string,
	totalBytes: number | undefined,
	contentType: string | undefined,
	options: VerifyAssetOptions
): Promise<VerifiedAsset> {
	const maxBytes = options.maxBytes ?? MAX_INPUT_BYTES;
	if (!Number.isSafeInteger(maxBytes) || maxBytes <= 0) {
		throw new Error('The fixture asset byte limit must be a positive integer.');
	}
	if (totalBytes !== undefined && totalBytes > maxBytes) {
		throw new Error(`Fixture asset exceeds the ${formatLimit(maxBytes)} browser limit.`);
	}
	const chunks: Uint8Array[] = [];
	let receivedBytes = 0;
	const reader = stream.getReader();
	try {
		while (true) {
			ensureNotAborted(options.signal);
			const { done, value } = await reader.read();
			if (done) break;
			const nextSize = receivedBytes + value.byteLength;
			if (!Number.isSafeInteger(nextSize) || nextSize > maxBytes) {
				await reader.cancel();
				throw new Error(`Fixture asset exceeds the ${formatLimit(maxBytes)} browser limit.`);
			}
			chunks.push(value);
			receivedBytes = nextSize;
			options.onProgress?.({ receivedBytes, totalBytes });
		}
	} finally {
		reader.releaseLock();
	}

	ensureNotAborted(options.signal);
	const bytes = concatenate(chunks, receivedBytes);
	const actualSha256 = await sha256Hex(bytes);
	if (actualSha256 !== expectedSha256.toLowerCase()) {
		throw new Error(`SHA-256 mismatch: expected ${expectedSha256}, received ${actualSha256}.`);
	}

	return { bytes, sha256: actualSha256, contentType };
}

function formatLimit(bytes: number): string {
	return bytes >= 1024 * 1024
		? `${Math.round(bytes / 1024 / 1024)} MiB`
		: `${bytes} byte${bytes === 1 ? '' : 's'}`;
}

function ownedBuffer(bytes: Uint8Array): ArrayBuffer {
	if (
		bytes.buffer instanceof ArrayBuffer &&
		bytes.byteOffset === 0 &&
		bytes.byteLength === bytes.buffer.byteLength
	) {
		return bytes.buffer;
	}
	return Uint8Array.from(bytes).buffer;
}

export async function sha256Hex(bytes: Uint8Array): Promise<string> {
	const digest = await crypto.subtle.digest('SHA-256', ownedBuffer(bytes));
	return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, '0')).join('');
}

export async function fetchVerifiedAsset(
	url: string,
	expectedSha256: string,
	options: FetchVerifiedOptions = {}
): Promise<VerifiedAsset> {
	ensureNotAborted(options.signal);
	const response = await (options.fetcher ?? fetch)(url, {
		signal: options.signal,
		cache: 'no-cache'
	});
	if (!response.ok) throw new Error(`Download failed with HTTP ${response.status}.`);

	const totalHeader = response.headers.get('content-length');
	const parsedTotal = totalHeader === null ? undefined : Number(totalHeader);
	const totalBytes =
		parsedTotal !== undefined && Number.isSafeInteger(parsedTotal) && parsedTotal >= 0
			? parsedTotal
			: undefined;
	if (response.body) {
		return readVerifiedStream(
			response.body,
			expectedSha256,
			totalBytes,
			response.headers.get('content-type') ?? undefined,
			options
		);
	}

	throw new Error('The fixture response cannot be read as a bounded stream.');
}

export async function readVerifiedFileAsset(
	file: File,
	expectedSha256: string,
	options: VerifyAssetOptions = {}
): Promise<VerifiedAsset> {
	ensureNotAborted(options.signal);
	const maxBytes = options.maxBytes ?? MAX_INPUT_BYTES;
	if (file.size > maxBytes) {
		throw new Error(`Fixture asset exceeds the ${formatLimit(maxBytes)} browser limit.`);
	}
	return readVerifiedStream(
		file.stream(),
		expectedSha256,
		file.size,
		file.type || undefined,
		options
	);
}

export function transferableBuffer(bytes: Uint8Array): ArrayBuffer {
	return ownedBuffer(bytes);
}
