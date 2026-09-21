/** Owns Canvas backing stores with a byte budget and least-recently-used eviction. */
export class CanvasCache<K, V extends { canvas: HTMLCanvasElement }> {
	private entries = new Map<
		K,
		{ value: V; bytes: number; onEvict?: () => void }
	>();
	bytes = 0;
	constructor(
		private budget: number,
		private maxEntries = Infinity
	) {}

	get(key: K): V | undefined {
		const entry = this.entries.get(key);
		if (!entry) return;
		this.entries.delete(key);
		this.entries.set(key, entry);
		return entry.value;
	}

	set(key: K, value: V, onEvict?: () => void): boolean {
		this.delete(key);
		const bytes = value.canvas.width * value.canvas.height * 4;
		if (bytes > this.budget) return false;
		while (
			this.bytes + bytes > this.budget ||
			this.entries.size >= this.maxEntries
		)
			this.delete(this.entries.keys().next().value!);
		this.entries.set(key, { value, bytes, onEvict });
		this.bytes += bytes;
		return true;
	}

	delete(key: K) {
		const entry = this.entries.get(key);
		if (!entry) return;
		this.entries.delete(key);
		this.bytes -= entry.bytes;
		entry.value.canvas.width = entry.value.canvas.height = 0;
		entry.onEvict?.();
	}

	clear() {
		for (const key of this.entries.keys()) this.delete(key);
	}
}
