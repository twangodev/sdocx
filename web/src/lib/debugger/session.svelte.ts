import { hexRows, type DebugQuery, type DebugRequest, type Index, type Replay, type Stroke } from './model';
import { DebuggerPlayback } from './playback.svelte';

/** Owns inspection request lifetimes; page/selection changes invalidate dependent work. */
export class DebuggerSession {
	index = $state.raw<Index | null>(null);
	page = $state(0);
	properties = $state.raw<Record<string, unknown>>({});
	selected = $state('');
	entry = $state<number | null>(null);
	byteOffset = $state(0);
	byteTotal = $state(0);
	hex = $state('');
	error = $state('');
	busy = $state(false);
	readonly playback: DebuggerPlayback;
	private alive = true;
	private pageRequest = 0;
	private selectionRequest = 0;
	private hexRequest = 0;
	private pendingSelection: number | null = null;

	constructor(
		readonly query: DebugQuery,
		private readonly navigate: (visiblePage: number) => void,
		private readonly onSelection: (showProperties: boolean) => void
	) {
		this.playback = new DebuggerPlayback((item) => {
			this.invalidateSelection();
			this.selected = JSON.stringify({ kind: 'object', page: this.page, offset: item.offset });
			this.properties = {
				offset: item.offset,
				timing: this.playback.tracks.find((track) => track.offset === item.offset)?.synthetic
					? 'Synthetic sample timing' : 'Recorded millisecond timing',
				stroke: item.stroke,
				rendering: { profile: item.geometry.profile, support: item.geometry.support }
			};
		});
	}

	async start(): Promise<void> {
		try {
			const result = await this.query<Index>({ kind: 'index' });
			if (!this.alive) return;
			this.index = result;
			this.byteTotal = result.sourceSize;
		} catch (cause) {
			if (this.alive) this.error = String(cause);
		}
	}

	destroy(): void {
		this.alive = false;
		++this.pageRequest;
		this.invalidateSelection();
		this.playback.playing = false;
	}

	private invalidateSelection(): void {
		this.pendingSelection = null;
		++this.selectionRequest;
		++this.hexRequest;
	}

	async readHex(offset = this.byteOffset): Promise<void> {
		const id = ++this.hexRequest;
		this.byteOffset = Math.max(0, offset);
		try {
			const result = await this.query<{ bytes: number[]; offset: number; total: number }>({
				kind: 'bytes', entry: this.entry, offset: this.byteOffset, length: 4096
			});
			if (!this.alive || id !== this.hexRequest) return;
			this.hex = hexRows(result.bytes, result.offset);
			this.byteTotal = result.total;
		} catch (cause) {
			if (this.alive && id === this.hexRequest) this.error = String(cause);
		}
	}

	async loadPage(next: number, navigate = true): Promise<void> {
		const visible = this.index?.pages[next]?.visibleIndex;
		if (navigate && visible != null) this.navigate(visible);
		const id = ++this.pageRequest;
		this.invalidateSelection();
		this.busy = true;
		this.error = '';
		this.page = next;
		this.playback.reset();
		this.selected = '';
		this.properties = {};
		this.hex = '';
		this.entry = null;
		this.byteOffset = 0;
		this.byteTotal = this.index?.sourceSize ?? 0;
		try {
			const result = await this.query<Replay>({ kind: 'replay', page: next });
			if (!this.alive || id !== this.pageRequest) return;
			this.playback.load(result);
			// A tree selection can complete before its page replay does.
			if (this.playback.selectedOffset !== null) {
				this.properties = this.renderingProperties(this.properties, this.playback.selectedOffset);
			}
		} catch (cause) {
			if (this.alive && id === this.pageRequest) this.error = String(cause);
		} finally {
			if (this.alive && id === this.pageRequest) this.busy = false;
		}
	}

	private renderingProperties(data: Record<string, unknown>, offset: number): Record<string, unknown> {
		const geometry = this.playback.replay?.strokes.find((stroke) => stroke.offset === offset)?.geometry;
		return geometry ? { ...data, rendering: { profile: geometry.profile, support: geometry.support } } : data;
	}

	selectRecord = (request: DebugRequest, data: Record<string, unknown>): void => {
		if (!this.alive) return;
		if ('page' in request && request.page !== this.page) void this.loadPage(request.page);
		this.invalidateSelection();
		this.selected = JSON.stringify(request);
		this.properties = data;
		this.playback.select(null);
		if (request.kind === 'object') {
			this.properties = this.renderingProperties(data, request.offset);
			const stroke = data.stroke && typeof data.stroke === 'object' && 'points' in data.stroke
				? data.stroke as Stroke : null;
			this.playback.select(request.offset, stroke);
		}
		if ('entry' in data && typeof data.entry === 'number') {
			this.entry = data.entry;
			void this.readHex(typeof data.offset === 'number' ? data.offset : 0);
		} else if (request.kind === 'entry') {
			this.entry = request.entry;
			void this.readHex(0);
		}
		this.onSelection(request.kind !== 'page' && request.kind !== 'layer');
	};

	choose = async (request: DebugRequest): Promise<Record<string, unknown> | null> => {
		const id = ++this.selectionRequest;
		++this.hexRequest;
		this.pendingSelection = id;
		try {
			const data = await this.query<Record<string, unknown>>(request);
			if (!this.alive || id !== this.selectionRequest) return null;
			this.selectRecord(request, data);
			return data;
		} catch (cause) {
			if (this.alive && id === this.selectionRequest) this.error = String(cause);
		} finally {
			if (this.pendingSelection === id) this.pendingSelection = null;
		}
		return null;
	};

	pick(offset: number): Promise<Record<string, unknown> | null> {
		return this.choose({ kind: 'object', page: this.page, offset });
	}

	step(direction: number): void {
		const offset = this.playback.step(direction);
		if (offset !== undefined) void this.pick(offset);
	}

	/** Called after the UI debounce; the cleanup cancels requests even without a new request. */
	inspectSelected(): () => void {
		const offset = this.playback.selectedOffset;
		if (offset === null || this.pendingSelection !== null) return () => {};
		const id = ++this.selectionRequest;
		const page = this.page;
		let cancelled = false;
		void (async () => {
			try {
				const data = await this.query<Record<string, unknown>>({ kind: 'object', page, offset });
				if (cancelled || !this.alive || id !== this.selectionRequest) return;
				this.properties = this.renderingProperties(data, offset);
				this.entry = data.entry as number;
				void this.readHex(offset);
			} catch (cause) {
				if (!cancelled && this.alive && id === this.selectionRequest) this.error = String(cause);
			}
		})();
		return () => { cancelled = true; };
	}

	selectOriginal(): void {
		this.invalidateSelection();
		this.entry = null;
		this.properties = {
			sourceSize: this.index?.sourceSize,
			zipLength: this.index?.zipLength,
			description: 'Original file, including appended end tag'
		};
		this.playback.select(null);
		void this.readHex(0);
		this.onSelection(true);
	}
}
