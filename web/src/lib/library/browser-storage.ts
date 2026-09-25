export interface StorageStatus {
	usage: number | null;
	quota: number | null;
	persisted: boolean | null;
	canRequestPersistence: boolean;
}

export async function storageStatus(): Promise<StorageStatus> {
	const manager = navigator.storage;
	const [estimate, persisted] = await Promise.all([
		manager?.estimate?.() ?? Promise.resolve({}),
		manager?.persisted?.() ?? Promise.resolve(null)
	]);
	return {
		usage: estimate.usage ?? null,
		quota: estimate.quota ?? null,
		persisted,
		canRequestPersistence: typeof manager?.persist === 'function'
	};
}

export async function requestPersistence(): Promise<boolean> {
	if (!navigator.storage?.persist)
		throw new Error('This browser does not support persistence requests.');
	return navigator.storage.persist();
}

export function formatStorageBytes(bytes: number | null): string {
	if (bytes === null) return 'Unavailable';
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
	if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MiB`;
	return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GiB`;
}
