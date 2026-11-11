import { ImportProcessor, type ImportOutcome, type ImportProgress } from './import-processor';
import { errorMessage, type LibrarySnapshot, type LibrarySort, type LibrarySource } from './model';
import { LibraryCatalog } from './catalog';
import { OpfsAssetStore } from './asset-store';
import { LibraryService } from './service';
import { visibleDocuments } from './view';
import { createThumbnail } from './thumbnail';
import { downloadBlob } from '$converter/files';

interface WorkspaceLifecycle {
	channel: BroadcastChannel | null;
	ready: Promise<void>;
	stop: () => void;
}

export class LibraryWorkspace {
	snapshot = $state.raw<LibrarySnapshot>({ documents: [], collections: [], memberships: [] });
	source = $state<LibrarySource>('all');
	search = $state('');
	sort = $state<LibrarySort>('newest');
	view = $state<'grid' | 'list'>('grid');
	selected = $state<string[]>([]);
	scrollTop = 0;
	loading = $state(true);
	importing = $state(false);
	progress = $state<ImportProgress | null>(null);
	results = $state.raw<ImportOutcome[]>([]);
	cancelled = $state(false);
	error = $state('');
	available = $state(false);
	private lifecycle: WorkspaceLifecycle | undefined;
	private importRun: object | undefined;
	private refreshGeneration = 0;
	private readonly restoringThumbnails = new Set<string>();
	readonly service = new LibraryService(
		new LibraryCatalog(),
		new OpfsAssetStore(),
		undefined,
		() => {
			this.lifecycle?.channel?.postMessage('changed');
		}
	);
	private readonly importer = new ImportProcessor((document, collectionId) => {
		if (!this.available)
			throw new Error('Local storage is unavailable. You can open this note temporarily.');
		return this.service.importDocument(document, collectionId);
	});

	get visible() {
		return visibleDocuments(this.snapshot, this.source, this.search, this.sort);
	}
	get collectionId() {
		return typeof this.source === 'object' ? this.source.collectionId : undefined;
	}
	get title() {
		return typeof this.source === 'object'
			? (this.snapshot.collections.find((collection) => collection.id === this.collectionId)
					?.name ?? 'Collection')
			: { all: 'All notes', recent: 'Recent', favorites: 'Favorites' }[this.source];
	}

	start(): () => void {
		this.lifecycle?.stop();
		const lifecycle: WorkspaceLifecycle = {
			channel: null,
			ready: Promise.resolve(),
			stop: () => {}
		};
		this.lifecycle = lifecycle;
		this.loading = true;
		this.available = false;
		this.error = '';
		const refresh = () => {
			if (this.lifecycle === lifecycle && this.available)
				void this.refreshAndReport(lifecycle);
		};
		if (typeof BroadcastChannel !== 'undefined') {
			lifecycle.channel = new BroadcastChannel('sdocx-library');
			lifecycle.channel.onmessage = () => {
				if (this.lifecycle === lifecycle) void this.refreshAndReport(lifecycle);
			};
		}
		window.addEventListener('focus', refresh);
		let stopped = false;
		lifecycle.stop = () => {
			if (stopped) return;
			stopped = true;
			lifecycle.channel?.close();
			window.removeEventListener('focus', refresh);
			if (this.lifecycle !== lifecycle) return;
			this.lifecycle = undefined;
			this.available = false;
			this.loading = false;
			this.importer.cancel();
			this.service.catalog.close();
		};
		lifecycle.ready = this.initialize(lifecycle);
		return lifecycle.stop;
	}

	private async initialize(lifecycle: WorkspaceLifecycle): Promise<void> {
		try {
			if (!navigator.storage?.getDirectory || !navigator.locks || !globalThis.indexedDB)
				throw new Error('Local storage is unavailable. Imported notes can be opened temporarily.');
			await this.service.catalog.open();
			if (this.lifecycle !== lifecycle) return;
			await this.service.recover();
			if (this.lifecycle !== lifecycle) return;
			this.available = true;
			await this.refreshAndReport(lifecycle);
		} catch (error) {
			if (this.lifecycle === lifecycle) this.error = errorMessage(error);
		} finally {
			if (this.lifecycle === lifecycle) this.loading = false;
		}
	}

	async refresh(): Promise<void> {
		const lifecycle = this.lifecycle;
		if (!lifecycle) return;
		await this.readSnapshot(lifecycle, ++this.refreshGeneration);
	}

	private async refreshAndReport(lifecycle: WorkspaceLifecycle): Promise<void> {
		const generation = ++this.refreshGeneration;
		try {
			await this.readSnapshot(lifecycle, generation);
		} catch (error) {
			if (this.lifecycle === lifecycle && generation === this.refreshGeneration)
				this.error = errorMessage(error);
		}
	}

	private async readSnapshot(lifecycle: WorkspaceLifecycle, generation: number): Promise<void> {
		const snapshot = await this.service.catalog.snapshot();
		if (this.lifecycle !== lifecycle || generation !== this.refreshGeneration) return;
		this.snapshot = snapshot;
		this.selected = this.selected.filter((id) =>
			snapshot.documents.some((document) => document.id === id)
		);
		if (
			this.collectionId &&
			!snapshot.collections.some((collection) => collection.id === this.collectionId)
		)
			this.source = 'all';
	}

	selectSource(source: LibrarySource): void {
		this.source = source;
		this.selected = [];
		this.scrollTop = 0;
	}

	toggleSelection(id: string): void {
		this.selected = this.selected.includes(id)
			? this.selected.filter((selected) => selected !== id)
			: [...this.selected, id];
	}

	async perform(action: () => Promise<unknown>): Promise<void> {
		const lifecycle = this.lifecycle;
		if (lifecycle) this.error = '';
		try {
			await action();
		} catch (error) {
			if (lifecycle && this.lifecycle === lifecycle) this.error = errorMessage(error);
		} finally {
			if (lifecycle && this.lifecycle === lifecycle && this.available)
				await this.refreshAndReport(lifecycle);
		}
	}

	async importFiles(files: File[]): Promise<ImportOutcome[]> {
		const lifecycle = this.lifecycle;
		if (!lifecycle) return [];
		await lifecycle.ready;
		if (this.lifecycle !== lifecycle || this.importRun || !files.length) return [];
		const run = {};
		this.importRun = run;
		const results: ImportOutcome[] = [];
		this.importing = true;
		this.error = '';
		this.results = [];
		this.cancelled = false;
		try {
			await this.importer.run(files, {
				collectionId: this.collectionId,
				onProgress: (progress) => {
					if (this.lifecycle === lifecycle) this.progress = progress;
				},
				onResult: (result) => {
					results.push(result);
					if (this.lifecycle === lifecycle) this.results = [...results];
				},
				approveLargeFile: (file) =>
					this.lifecycle === lifecycle && window.confirm(
						`${file.name} is over 100 MiB. Parsing may use substantial memory. Import locally?`
					)
			});
		} catch (error) {
			if (this.lifecycle === lifecycle) this.error = errorMessage(error);
		} finally {
			if (this.importRun === run) {
				this.importRun = undefined;
				this.importing = false;
			}
			if (this.lifecycle === lifecycle && this.available)
				await this.refreshAndReport(lifecycle);
		}
		return results;
	}

	async downloadOriginal(id: string): Promise<void> {
		await this.perform(async () => {
			const file = await this.service.openDocument(id);
			downloadBlob(file, file.name);
		});
	}

	async restoreThumbnail(id: string, render: () => Promise<string>): Promise<void> {
		if (this.restoringThumbnails.has(id) || this.snapshot.documents.find((document) => document.id === id)?.thumbnail) return;
		this.restoringThumbnails.add(id);
		const lifecycle = this.lifecycle;
		try {
			await this.service.cacheThumbnail(id, await createThumbnail(await render()));
			if (lifecycle && this.lifecycle === lifecycle) await this.refresh();
		} catch {
			return;
		} finally {
			this.restoringThumbnails.delete(id);
		}
	}

	cancelImport(): void {
		this.cancelled = true;
		this.importer.cancel();
	}
}
