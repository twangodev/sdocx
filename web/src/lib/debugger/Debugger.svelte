<script lang="ts">
	import {
		Bug,
		FolderTree,
		X,
		SlidersHorizontal,
		FileJson,
		Binary,
		Files,
		Archive,
		File,
		ListPlus,
		RotateCcw,
		StepBack,
		Play,
		Pause,
		StepForward,
		ChevronLeft,
		ChevronRight,
		Search,
		Gauge,
		ScanSearch,
		PenLine
	} from '@lucide/svelte';
	import IconButton from '$lib/components/ui/IconButton.svelte';
	import { onMount, untrack } from 'svelte';
	import type { DocumentSession } from '$converter/document-session.svelte';
	import RecordTree from './RecordTree.svelte';
	import ValueTree from './ValueTree.svelte';
	import {
		timeline,
		hexRows,
		sampleAt,
		type DebugRequest,
		type DebugPreview,
		type DebugQuery,
		type Index,
		type Replay,
		type Stroke
	} from './model';
	let {
		session,
		viewerPage,
		onPageChange,
		onPreviewChange,
		onClose
	}: {
		session: DocumentSession;
		viewerPage: number;
		onPageChange: (page: number) => void;
		onPreviewChange: (preview: DebugPreview | null) => void;
		onClose: () => void;
	} = $props();
	const query: DebugQuery = async <T,>(r: DebugRequest) =>
		(await session.debug(r)) as T;
	let index = $state.raw<Index | null>(null),
		replay = $state.raw<Replay | null>(null);
	let page = $state(0),
		properties = $state.raw<Record<string, unknown>>({}),
		selected = $state('');
	let selectedOffset = $state<number | null>(null),
		sampleIndex = $state(-1),
		sampleScroll = $state(0);
	let selectedStroke = $state.raw<Stroke | null>(null);
	let entry = $state<number | null>(null),
		byteOffset = $state(0),
		byteTotal = $state(0),
		hex = $state('');
	let search = $state(''),
		mobileTab = $state('Tree'),
		error = $state(''),
		busy = $state(false);
	let playing = $state(false),
		position = $state(0),
		speed = $state(1);
	let alive = true,
		pageRequest = 0,
		selectionRequest = 0,
		hexRequest = 0;
	const tracks = $derived(timeline(replay?.strokes ?? []));
	const duration = $derived(tracks.at(-1)?.end ?? 0);
	const sampleStart = $derived(Math.floor(sampleScroll / 26));
	const samples = $derived(selectedStroke?.points ?? []);
	const filteredPages = $derived(
		index?.pages.filter((p) =>
			`${p.name} ${p.id}`.toLowerCase().includes(search.toLowerCase())
		) ?? []
	);
	const filteredEntries = $derived(
		index?.entries.filter((e) =>
			e.name.toLowerCase().includes(search.toLowerCase())
		) ?? []
	);
	let entryCount = $state(100),
		pageCount = $state(100);
	async function readHex(offset = byteOffset) {
		const id = ++hexRequest;
		byteOffset = Math.max(0, offset);
		try {
			const result = await query<{
				bytes: number[];
				offset: number;
				total: number;
			}>({ kind: 'bytes', entry, offset: byteOffset, length: 4096 });
			if (!alive || id !== hexRequest) return;
			hex = hexRows(result.bytes, result.offset);
			byteTotal = result.total;
		} catch (e) {
			if (alive && id === hexRequest) error = String(e);
		}
	}
	async function loadPage(next: number, navigate = true) {
		const visible = index?.pages[next]?.visibleIndex;
		if (navigate && visible != null && visible !== viewerPage)
			onPageChange(visible);
		const id = ++pageRequest;
		playing = false;
		busy = true;
		error = '';
		page = next;
		replay = null;
		selectedStroke = null;
		selectedOffset = null;
		sampleIndex = -1;
		position = 0;
		try {
			const result = await query<Replay>({ kind: 'replay', page: next });
			if (!alive || id !== pageRequest) return;
			replay = result;
			position = timeline(result.strokes).at(-1)?.end ?? 0;
		} catch (e) {
			if (alive && id === pageRequest) error = String(e);
		} finally {
			if (alive && id === pageRequest) busy = false;
		}
	}
	function renderingProperties(data: Record<string, unknown>, offset: number) {
		const geometry = replay?.strokes.find((s) => s.offset === offset)?.geometry;
		return geometry
			? { ...data, rendering: { profile: geometry.profile, support: geometry.support } }
			: data;
	}
	function selectRecord(request: DebugRequest, data: Record<string, unknown>) {
		if (!alive) return;
		++selectionRequest;
		playing = false;
		selected = JSON.stringify(request);
		properties = data;
		selectedStroke = null;
		sampleIndex = -1;
		sampleScroll = 0;
		if ('page' in request && request.page !== page) void loadPage(request.page);
		if (request.kind === 'object') {
			selectedOffset = request.offset;
			if (request.page === page) properties = renderingProperties(data, request.offset);
			selectedStroke =
				data.stroke &&
				typeof data.stroke === 'object' &&
				'points' in data.stroke
					? (data.stroke as Stroke)
					: null;
			const track = tracks.find((t) => t.offset === request.offset);
			if (track && request.page === page) position = track.end;
		} else selectedOffset = null;
		if ('entry' in data && typeof data.entry === 'number') {
			entry = data.entry;
			void readHex(typeof data.offset === 'number' ? data.offset : 0);
		} else if (request.kind === 'entry') {
			entry = request.entry;
			void readHex(0);
		}
		if (request.kind !== 'page' && request.kind !== 'layer')
			mobileTab = 'Properties';
	}
	async function choose(request: DebugRequest) {
		const id = ++selectionRequest;
		try {
			const data = await query<Record<string, unknown>>(request);
			if (alive && id === selectionRequest) selectRecord(request, data);
		} catch (e) {
			if (alive && id === selectionRequest) error = String(e);
		}
	}
	async function pick(offset: number) {
		await choose({ kind: 'object', page, offset });
	}
	function scrub(value: number) {
		playing = false;
		position = value;
		syncSample();
	}
	function syncSample() {
		const track =
			tracks.find((t) => position >= t.start && position <= t.end) ??
			tracks.findLast((t) => t.end <= position);
		if (track) {
			if (selectedOffset !== track.offset) {
				const item = replay?.strokes.find((s) => s.offset === track.offset);
				properties = {
					offset: track.offset,
					timing: track.synthetic
						? 'Synthetic sample timing'
						: 'Recorded millisecond timing',
					stroke: item?.stroke,
					rendering: item
						? { profile: item.geometry.profile, support: item.geometry.support }
						: null
				};
				selected = JSON.stringify({
					kind: 'object',
					page,
					offset: track.offset
				});
			}
			selectedOffset = track.offset;
			selectedStroke =
				replay?.strokes.find((s) => s.offset === track.offset)?.stroke ?? null;
			sampleIndex = sampleAt(track.times, position - track.start);
		}
	}
	function selectSample(i: number) {
		playing = false;
		sampleIndex = i;
		const track = tracks.find((t) => t.offset === selectedOffset);
		if (track) position = track.start + track.times[i];
	}
	function step(direction: number) {
		playing = false;
		const current = tracks.findIndex((t) => t.offset === selectedOffset);
		const track =
			tracks[Math.max(0, Math.min(tracks.length - 1, current + direction))];
		if (track) {
			position = track.end;
			void pick(track.offset);
		}
	}

	// Debounce metadata/hex inspection while scrubbing; geometry stays local.
	$effect(() => {
		const offset = selectedOffset;
		const sourcePage = page;
		if (offset === null) return;
		let cancelled = false;
		const timer = setTimeout(async () => {
			try {
				const data = await query<Record<string, unknown>>({
					kind: 'object',
					page: sourcePage,
					offset
				});
				if (cancelled || !alive) return;
				properties = renderingProperties(data, offset);
				entry = data.entry as number;
				void readHex(offset);
			} catch (cause) {
				if (!cancelled && alive) error = String(cause);
			}
		}, 120);
		return () => {
			cancelled = true;
			clearTimeout(timer);
		};
	});
	$effect(() => {
		if (!playing) return;
		let frame = 0,
			last = performance.now();
		function tick(now: number) {
			const elapsed = now - last;
			last = now;
			position = Math.min(duration, position + elapsed * speed);
			syncSample();
			if (position >= duration) {
				playing = false;
				return;
			}
			frame = requestAnimationFrame(tick);
		}
		frame = requestAnimationFrame(tick);
		return () => cancelAnimationFrame(frame);
	});
	$effect(() => {
		const source = index?.pages.find((p) => p.visibleIndex === viewerPage);
		untrack(() => {
			if (source && (source.index !== page || (!replay && !busy)))
				void loadPage(source.index, false);
		});
	});
	$effect(() => {
		const visibleIndex = index?.pages[page]?.visibleIndex;
		onPreviewChange(
			replay && visibleIndex != null
				? {
						pageIndex: visibleIndex,
						sourcePage: page,
						replay,
						tracks,
						position,
						selectedOffset,
						sampleIndex,
						onSelect: (offset) => void pick(offset)
					}
				: null
		);
	});
	onMount(() => {
		void (async () => {
			try {
				const result = await query<Index>({ kind: 'index' });
				if (!alive) return;
				index = result;
				byteTotal = result.sourceSize;
			} catch (e) {
				if (alive) error = String(e);
			}
		})();
		return () => {
			alive = false;
			onPreviewChange(null);
			playing = false;
			++pageRequest;
			++selectionRequest;
			++hexRequest;
		};
	});
</script>

<aside id="debugger-sidebar" class="debugger" aria-label="File debugger">
	<div class="heading">
		<strong class="icon-label"
			><Bug size={14} aria-hidden="true" />Debugger</strong
		>
		<IconButton label="Close debugger" tooltip onclick={onClose}
			><X size={14} aria-hidden="true" /></IconButton
		>
	</div>
	<label class="page-picker"
		>Stored page <select
			aria-label="Debugger page"
			value={page}
			onchange={(e) => void loadPage(Number(e.currentTarget.value))}
		>
			{#each index?.pages ?? [] as p}<option value={p.index}
					>{p.index + 1} · {p.id.slice(0, 8)}</option
				>{/each}
		</select></label
	>
	{#if index?.pages[page]?.visibleIndex === null}<p class="notice">
			This stored page has no visible preview.
		</p>{/if}
	{#if error}<p role="alert" class="error">{error}</p>{/if}
	<div class="timeline">
		<div class="controls">
			<IconButton
				label="Restart replay"
				tooltip
				disabled={!tracks.length}
				onclick={() => scrub(0)}
				><RotateCcw
					size={14}
					strokeWidth={1.5}
					aria-hidden="true"
				/></IconButton
			>
			<IconButton
				label="Previous stroke"
				tooltip
				disabled={!tracks.length}
				onclick={() => step(-1)}
				><StepBack size={14} strokeWidth={1.5} aria-hidden="true" /></IconButton
			>
			<IconButton
				label={playing ? 'Pause' : 'Play'}
				tooltip
				disabled={!duration}
				onclick={() => {
					if (position >= duration) position = 0;
					playing = !playing;
				}}
			>
				{#if playing}<Pause
						size={14}
						strokeWidth={1.5}
						aria-hidden="true"
					/>{:else}<Play size={14} strokeWidth={1.5} aria-hidden="true" />{/if}
			</IconButton>
			<IconButton
				label="Next stroke"
				tooltip
				disabled={!tracks.length}
				onclick={() => step(1)}
				><StepForward
					size={14}
					strokeWidth={1.5}
					aria-hidden="true"
				/></IconButton
			>
			<Gauge size={13} aria-hidden="true" />
			<select aria-label="Playback speed" bind:value={speed}
				>{#each [0.25, 0.5, 1, 2, 4] as rate}<option value={rate}
						>{rate}×</option
					>{/each}</select
			>
			<span
				>{(position / 1000).toFixed(2)} / {(duration / 1000).toFixed(2)} s</span
			>
		</div>
		<input
			aria-label="Replay position"
			type="range"
			min="0"
			max={duration || 1}
			step="1"
			value={position}
			disabled={!duration}
			oninput={(e) => scrub(Number(e.currentTarget.value))}
		/>
		<p>
			{tracks.length} strokes · 150 ms synthetic gaps{tracks.some(
				(t) => t.synthetic
			)
				? ' · unverified timing uses 16 ms/sample'
				: ''}. Stored order, not edit history.
		</p>
	</div>
	<nav class="tabs" aria-label="Debugger panels">
		{#each ['Tree', 'Properties'] as tab}<button
				aria-pressed={mobileTab === tab}
				onclick={() => (mobileTab = tab)}
			>
				{#if tab === 'Tree'}<FolderTree
						size={14}
						aria-hidden="true"
					/>{:else}<SlidersHorizontal size={14} aria-hidden="true" />{/if}{tab}
			</button>{/each}
	</nav>
	<div class="panel">
		<div hidden={mobileTab !== 'Tree'}>
			<aside aria-label="File structure">
				<div class="search-field">
					<Search size={13} aria-hidden="true" />
					<input
						aria-label="Search file tree"
						placeholder="Filter entries and pages"
						bind:value={search}
					/>
				</div>
				<button onclick={() => void choose({ kind: 'document' })}
					><FileJson size={14} aria-hidden="true" />Document metadata &
					diagnostics</button
				>
				<button
					onclick={() => {
						entry = null;
						properties = {
							sourceSize: index?.sourceSize,
							zipLength: index?.zipLength,
							description: 'Original file, including appended end tag'
						};
						selectedOffset = null;
						selectedStroke = null;
						void readHex(0);
						mobileTab = 'Properties';
					}}><Binary size={14} aria-hidden="true" />Original file bytes</button
				>
				<h3><Files size={13} aria-hidden="true" />Stored pages</h3>
				{#each filteredPages.slice(0, pageCount) as p (p.index)}<RecordTree
						label={`Page ${p.index + 1} · ${p.id.slice(0, 8)}`}
						request={{ kind: 'page', page: p.index }}
						{query}
						onselect={selectRecord}
						{selected}
					/>{/each}
				{#if filteredPages.length > pageCount}<button
						onclick={() => (pageCount += 100)}
						><ListPlus size={13} aria-hidden="true" />More pages</button
					>{/if}
				<h3><Archive size={13} aria-hidden="true" />Archive entries</h3>
				{#each filteredEntries.slice(0, entryCount) as e (e.index)}<button
						class="entry"
						onclick={() => void choose({ kind: 'entry', entry: e.index })}
						><File size={14} aria-hidden="true" /><span
							>{e.name}<small>{e.size.toLocaleString()} bytes</small></span
						></button
					>{/each}
				{#if filteredEntries.length > entryCount}<button
						onclick={() => (entryCount += 100)}
						><ListPlus size={13} aria-hidden="true" />More entries</button
					>{/if}
			</aside>
		</div>
		<div hidden={mobileTab !== 'Properties'}>
			<aside class="properties" aria-label="Record inspector">
				<h3>
					<ScanSearch size={13} aria-hidden="true" />Selected record {selectedOffset !==
					null
						? `@ 0x${selectedOffset.toString(16)}`
						: ''}
				</h3>
				{#key selected}<ValueTree value={properties} />{/key}
				{#if selectedStroke}
					<h3>
						<PenLine size={13} aria-hidden="true" />Stroke samples · {samples.length}
					</h3>
					{#if samples.length}
						<label class="controls"
							>Sample
							<input
								aria-label="Sample index"
								type="number"
								min="0"
								max={samples.length - 1}
								value={Math.max(0, sampleIndex)}
								onchange={(event) =>
									selectSample(
										Math.max(
											0,
											Math.min(
												samples.length - 1,
												Number(event.currentTarget.value)
											)
										)
									)}
							/>
							<span
								>t = {selectedStroke.timestamps[Math.max(0, sampleIndex)] ??
									'unknown'} (raw)</span
							>
						</label>
					{/if}
					<p>Coordinates · pressure · raw time · tilt · orientation</p>
					<div
						class="samples"
						onscroll={(e) => (sampleScroll = e.currentTarget.scrollTop)}
					>
						<div
							style:height={`${samples.length * 26}px`}
							class="sample-spacer"
						>
							<div style:transform={`translateY(${sampleStart * 26}px)`}>
								{#each samples.slice(sampleStart, sampleStart + 12) as point, i}
									{@const n = sampleStart + i}
									<button
										class:active={sampleIndex === n}
										onclick={() => selectSample(n)}
										title={`Sample ${n}`}
									>
										{n}: {point.x.toFixed(2)}, {point.y.toFixed(2)} · {selectedStroke.pressures[
											n
										]?.toFixed(3) ?? '—'} · {selectedStroke.timestamps[n] ??
											'—'} ·
										{selectedStroke.tilts[n]?.toFixed(3) ?? '—'} · {selectedStroke.orientations[
											n
										]?.toFixed(3) ?? '—'}
									</button>
								{/each}
							</div>
						</div>
					</div>
				{/if}
				<h3>
					<Binary size={13} aria-hidden="true" />Hex / ASCII · {entry === null
						? 'Original file'
						: `Entry ${entry}`}
				</h3>
				<div class="controls">
					<IconButton
						label="Previous bytes"
						tooltip
						disabled={byteOffset === 0}
						onclick={() => void readHex(Math.max(0, byteOffset - 4096))}
						><ChevronLeft size={14} aria-hidden="true" /></IconButton
					>
					<label
						>Offset <input
							aria-label="Byte offset"
							type="number"
							min="0"
							max={byteTotal}
							bind:value={byteOffset}
							onchange={() => void readHex()}
						/></label
					>
					<IconButton
						label="Next bytes"
						tooltip
						disabled={byteOffset + 4096 >= byteTotal}
						onclick={() => void readHex(byteOffset + 4096)}
						><ChevronRight size={14} aria-hidden="true" /></IconButton
					>
				</div>
				<pre class="hex">{hex ||
						'Select a file entry or object to inspect its bytes.'}</pre>
			</aside>
		</div>
	</div>
</aside>

<style>
	.debugger {
		display: flex;
		flex-direction: column;
		width: 360px;
		max-width: 100%;
		flex-shrink: 0;
		min-height: 0;
		border-left: 1px solid var(--color-subtle);
		background: var(--color-bg);
		font-size: 11px;
	}
	.heading {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 10px;
		border-bottom: 1px solid var(--color-subtle);
	}
	.icon-label,
	h3 {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	h3 {
		font-weight: 600;
		margin: 14px 0 8px;
	}
	.page-picker {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 10px;
	}
	.tabs {
		display: flex;
		gap: 6px;
		padding: 8px 10px;
		border-bottom: 1px solid var(--color-subtle);
	}
	.tabs button {
		display: flex;
		align-items: center;
		gap: 5px;
		flex: 1;
		justify-content: center;
	}
	.tabs button[aria-pressed='true'] {
		background: var(--color-surface);
		color: var(--color-text);
	}
	.panel {
		flex: 1;
		overflow: auto;
		min-height: 0;
		padding: 10px;
	}
	.panel aside {
		min-width: 0;
	}
	button,
	select,
	input {
		border: 1px solid var(--color-subtle);
		border-radius: 4px;
		padding: 5px 7px;
		background: transparent;
		color: inherit;
	}
	button {
		cursor: pointer;
	}
	button:hover {
		background: #8882;
	}
	button:disabled {
		opacity: 0.4;
		cursor: default;
	}
	button :global(svg) {
		flex-shrink: 0;
	}
	aside > button {
		display: flex;
		align-items: center;
		gap: 6px;
		text-align: left;
		width: 100%;
		margin-top: 5px;
	}
	.entry {
		overflow-wrap: anywhere;
	}
	.entry small {
		display: block;
		opacity: 0.6;
	}
	.search-field {
		position: relative;
	}
	.search-field :global(svg) {
		position: absolute;
		left: 7px;
		top: 9px;
		color: var(--color-muted);
	}
	.search-field input {
		width: 100%;
		padding-left: 25px;
	}
	.timeline {
		padding: 10px;
		border-bottom: 1px solid var(--color-subtle);
	}
	.controls {
		display: flex;
		align-items: center;
		gap: 6px;
		flex-wrap: wrap;
	}
	.controls input {
		width: 90px;
	}
	input[type='range'] {
		width: 100%;
		margin: 8px 0;
		padding: 0;
	}
	.timeline p,
	.properties p,
	.notice {
		font-size: 10px;
		color: var(--color-muted);
	}
	.notice,
	.error {
		padding: 8px;
	}
	.error {
		color: #d44;
	}
	.hex {
		font-size: 10px;
		overflow: auto;
		max-height: 320px;
		margin-top: 10px;
		white-space: pre;
	}
	.samples {
		height: 260px;
		overflow: auto;
		border: 1px solid var(--color-subtle);
	}
	.sample-spacer {
		position: relative;
	}
	.samples button {
		height: 26px;
		padding: 2px 4px;
		display: block;
		white-space: nowrap;
		min-width: 100%;
		text-align: left;
		border: 0;
		font: 10px monospace;
	}
	.samples .active {
		background: #2684ff33;
	}
	@media (max-width: 900px) {
		.debugger {
			position: absolute;
			right: 0;
			top: 0;
			bottom: 0;
			z-index: 30;
			width: min(360px, 90vw);
			box-shadow: -12px 0 30px #0003;
		}
	}
</style>
