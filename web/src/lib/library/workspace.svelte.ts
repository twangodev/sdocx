import { ImportProcessor, type ImportOutcome, type ImportProgress } from './import-processor';
import { errorMessage, type LibrarySnapshot, type LibrarySort, type LibrarySource } from './model';
import { LibraryCatalog } from './catalog';
import { OpfsAssetStore } from './asset-store';
import { LibraryService } from './service';
import { visibleDocuments } from './view';
import { createThumbnail } from './thumbnail';

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
	private channel: BroadcastChannel | null = null;
	private refreshGeneration = 0;
	private stopped = false;
	private ready: Promise<void> = Promise.resolve();
	readonly service = new LibraryService(
		new LibraryCatalog(),
		new OpfsAssetStore(),
		undefined,
		() => {
			this.channel?.postMessage('changed');
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
		this.stopped = false;
		if (typeof BroadcastChannel !== 'undefined') {
			this.channel = new BroadcastChannel('sdocx-library');
			this.channel.onmessage = () => {
				void this.refresh().catch((error) => {
					this.error = errorMessage(error);
				});
			};
		}
		const refresh = () => {
			if (this.available)
				void this.refresh().catch((error) => {
					this.error = errorMessage(error);
				});
		};
		window.addEventListener('focus', refresh);
		this.ready = this.initialize();
		return () => {
			this.stopped = true;
			this.importer.cancel();
			this.channel?.close();
			this.channel = null;
			window.removeEventListener('focus', refresh);
			this.service.catalog.close();
		};
	}

	private async initialize(): Promise<void> {
		try {
			if (!navigator.storage?.getDirectory || !navigator.locks || !globalThis.indexedDB)
				throw new Error('Local storage is unavailable. Imported notes can be opened temporarily.');
			await this.service.catalog.open();
			await this.service.recover();
			if (this.stopped) return;
			this.available = true;
			await this.refresh();
		} catch (error) {
			this.error = errorMessage(error);
		} finally {
			this.loading = false;
		}
	}

	async refresh(): Promise<void> {
		const generation = ++this.refreshGeneration;
		const snapshot = await this.service.catalog.snapshot();
		if (this.stopped || generation !== this.refreshGeneration) return;
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
		this.error = '';
		try {
			await action();
		} catch (error) {
			this.error = errorMessage(error);
		} finally {
			if (this.available)
				await this.refresh().catch((error) => {
					this.error = errorMessage(error);
				});
		}
	}

	async importFiles(files: File[]): Promise<ImportOutcome[]> {
		await this.ready;
		if (this.importing || !files.length) return [];
		this.importing = true;
		this.error = '';
		this.results = [];
		this.cancelled = false;
		try {
			await this.importer.run(files, {
				collectionId: this.collectionId,
				onProgress: (progress) => {
					this.progress = progress;
				},
				onResult: (result) => {
					this.results = [...this.results, result];
				},
				approveLargeFile: (file) =>
					window.confirm(
						`${file.name} is over 100 MiB. Parsing may use substantial memory. Import locally?`
					)
			});
		} catch (error) {
			this.error = errorMessage(error);
		} finally {
			this.importing = false;
			if (this.available)
				await this.refresh().catch((error) => {
					this.error = errorMessage(error);
				});
		}
		return this.results;
	}

	async restoreThumbnail(id: string, svg: string): Promise<void> {
		if (this.snapshot.documents.find((document) => document.id === id)?.thumbnail) return;
		try {
			await this.service.cacheThumbnail(id, await createThumbnail(svg));
			await this.refresh();
		} catch {
			return;
		}
	}

	cancelImport(): void {
		this.cancelled = true;
		this.importer.cancel();
	}
}
