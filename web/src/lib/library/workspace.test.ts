import 'fake-indexeddb/auto';
import Dexie from 'dexie';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LibraryWorkspace } from './workspace.svelte';
import { ImportProcessor, type ImportOutcome } from './import-processor';
import type { LibrarySnapshot } from './model';

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (error: Error) => void;
	const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
	return { promise, resolve, reject };
}
const empty: LibrarySnapshot = { documents: [], collections: [], memberships: [] };
const stops: (() => void)[] = [];
let browser: EventTarget;
let channels: { close: ReturnType<typeof vi.fn>; onmessage: (() => void) | null }[];

beforeEach(() => {
	browser = new EventTarget();
	channels = [];
	vi.stubGlobal('window', browser);
	vi.stubGlobal('navigator', { storage: { getDirectory: vi.fn() }, locks: {} });
	vi.stubGlobal('BroadcastChannel', class {
		close = vi.fn();
		postMessage = vi.fn();
		onmessage = null;
		constructor() { channels.push(this); }
	});
});
afterEach(() => {
	for (const stop of stops.splice(0)) stop();
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
});
function harness() {
	const workspace = new LibraryWorkspace();
	const catalog = workspace.service.catalog;
	const open = vi.spyOn(catalog, 'open').mockResolvedValue(catalog);
	const close = vi.spyOn(catalog, 'close').mockImplementation(() => {});
	const recover = vi.spyOn(workspace.service, 'recover').mockResolvedValue();
	const snapshot = vi.spyOn(catalog, 'snapshot').mockResolvedValue(empty);
	const start = () => {
		const stop = workspace.start();
		stops.push(stop);
		return stop;
	};
	return { workspace, open, close, recover, snapshot, start };
}

describe('library workspace lifecycle', () => {
	it('ignores obsolete initialization failure and finalization during restart', async () => {
		const { workspace, recover, start } = harness();
		const oldRecovery = deferred<void>(), newRecovery = deferred<void>();
		const oldRecoveryStarted = deferred<void>(), newRecoveryStarted = deferred<void>();
		recover.mockImplementationOnce(() => {
			oldRecoveryStarted.resolve();
			return oldRecovery.promise;
		}).mockImplementationOnce(() => {
			newRecoveryStarted.resolve();
			return newRecovery.promise;
		});
		start();
		const oldInitialization = workspace.importFiles([]);
		await oldRecoveryStarted.promise;
		start();
		await newRecoveryStarted.promise;
		oldRecovery.reject(new Error('obsolete recovery'));
		await oldInitialization;
		expect(workspace.error).toBe('');
		expect(workspace.loading).toBe(true);
		expect(workspace.available).toBe(false);
		newRecovery.resolve();
		await workspace.importFiles([]);
		expect(workspace.loading).toBe(false);
		expect(workspace.available).toBe(true);
	});

	it('does not continue recovery after an obsolete catalog open finishes', async () => {
		const { workspace, open, recover, start } = harness();
		const oldOpen = deferred<typeof workspace.service.catalog>();
		open.mockReturnValueOnce(Dexie.Promise.resolve(oldOpen.promise));
		start();
		const oldInitialization = workspace.importFiles([]);
		start();
		await workspace.importFiles([]);
		oldOpen.resolve(workspace.service.catalog);
		await oldInitialization;
		expect(recover).toHaveBeenCalledOnce();
		expect(workspace.available).toBe(true);
	});

	it('makes old cleanup idempotent without closing a newer lifecycle resources', async () => {
		const { workspace, close, start } = harness();
		const stopOld = start();
		await workspace.importFiles([]);
		start();
		await workspace.importFiles([]);
		stopOld();
		expect(channels[0].close).toHaveBeenCalledOnce();
		expect(channels[1].close).not.toHaveBeenCalled();
		expect(close).toHaveBeenCalledOnce();
		expect(workspace.available).toBe(true);
	});

	it('starts a snapshot refresh when the active browser regains focus', async () => {
		const { workspace, snapshot, start } = harness();
		start(); await workspace.importFiles([]);
		snapshot.mockClear();
		browser.dispatchEvent(new Event('focus'));
		expect(snapshot).toHaveBeenCalledOnce();
	});

	it('admits reported refresh failures only for the latest request', async () => {
		const { workspace, snapshot, start } = harness();
		start(); await workspace.importFiles([]);
		const older = deferred<LibrarySnapshot>(), newer = deferred<LibrarySnapshot>();
		const olderStarted = deferred<void>(), newerStarted = deferred<void>();
		snapshot.mockImplementationOnce(() => {
			olderStarted.resolve();
			return older.promise;
		}).mockImplementationOnce(() => {
			newerStarted.resolve();
			return newer.promise;
		});
		const olderAction = workspace.perform(async () => {});
		await olderStarted.promise;
		const newerAction = workspace.perform(async () => {});
		await newerStarted.promise;
		newer.reject(new Error('latest snapshot'));
		await newerAction;
		older.reject(new Error('obsolete snapshot'));
		await olderAction;
		expect(workspace.error).toBe('latest snapshot');
	});

	it('keeps public refresh rejection while ignoring obsolete snapshot publication', async () => {
		const { workspace, snapshot, start } = harness();
		const stop = start(); await workspace.importFiles([]);
		const pending = deferred<LibrarySnapshot>();
		snapshot.mockReturnValueOnce(pending.promise);
		const refreshing = workspace.refresh();
		stop(); start(); await workspace.importFiles([]);
		pending.reject(new Error('obsolete public refresh'));
		await expect(refreshing).rejects.toThrow('obsolete public refresh');
		expect(workspace.error).toBe('');
		expect(workspace.snapshot).toBe(empty);
	});

	it('does not start an import that was waiting on an obsolete initialization', async () => {
		const { workspace, recover, start } = harness();
		const pending = deferred<void>();
		const recoveryStarted = deferred<void>();
		recover.mockImplementationOnce(() => {
			recoveryStarted.resolve();
			return pending.promise;
		});
		const run = vi.spyOn(ImportProcessor.prototype, 'run').mockResolvedValue([]);
		const stop = start();
		await recoveryStarted.promise;
		const importing = workspace.importFiles([{} as File]);
		stop(); start(); await workspace.importFiles([]);
		pending.resolve();
		expect(await importing).toEqual([]);
		expect(run).not.toHaveBeenCalled();
		expect(workspace.importing).toBe(false);
	});

	it('lets an admitted import settle across restart without stale presentation or overlapping runs', async () => {
		const { workspace, snapshot, start } = harness();
		const pending = deferred<ImportOutcome[]>();
		let options!: NonNullable<Parameters<ImportProcessor['run']>[1]>;
		const importStarted = deferred<void>();
		const run = vi.spyOn(ImportProcessor.prototype, 'run').mockImplementation((_files, next) => {
			options = next ?? {};
			importStarted.resolve();
			return pending.promise;
		});
		start(); await workspace.importFiles([]);
		const importing = workspace.importFiles([{} as File]);
		await importStarted.promise;
		start(); await workspace.importFiles([]);
		const reads = snapshot.mock.calls.length;
		expect(await workspace.importFiles([{} as File])).toEqual([]);
		expect(run).toHaveBeenCalledOnce();
		expect(workspace.importing).toBe(true);
		options.onProgress?.({ completed: 1, total: 1, filename: 'old.sdocx' });
		const result = { filename: 'old.sdocx', status: 'skipped' } as const;
		options.onResult?.(result);
		pending.resolve([result]);
		expect(await importing).toEqual([result]);
		expect(workspace.results).toEqual([]);
		expect(workspace.progress).toBeNull();
		expect(workspace.importing).toBe(false);
		expect(snapshot).toHaveBeenCalledTimes(reads);
	});

	it('lets storage actions settle without reporting obsolete errors or refreshing a restarted workspace', async () => {
		const { workspace, snapshot, start } = harness();
		start(); await workspace.importFiles([]);
		const action = deferred<void>();
		const performing = workspace.perform(() => action.promise);
		start(); await workspace.importFiles([]);
		const reads = snapshot.mock.calls.length;
		action.reject(new Error('obsolete action'));
		await performing;
		expect(workspace.error).toBe('');
		expect(snapshot).toHaveBeenCalledTimes(reads);
	});
});
