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
	import { DebuggerSession } from './session.svelte';
	import {
		INTER_STROKE_GAP_MS,
		SYNTHETIC_SAMPLE_INTERVAL_MS,
		type DebugPreview,
		type DebugQuery
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
	let search = $state('');
	let mobileTab = $state('Tree');
	let sampleScroll = $state(0);
	let entryCount = $state(100), pageCount = $state(100);
	const query: DebugQuery = async <T,>(request: Parameters<DebugQuery>[0]) =>
		(await session.debug(request)) as T;
	const inspector = new DebuggerSession(query, (page) => {
		if (page !== viewerPage) onPageChange(page);
	}, (showProperties) => {
		sampleScroll = 0;
		if (showProperties) mobileTab = 'Properties';
	});
	const playback = inspector.playback;
	const sampleStart = $derived(Math.floor(sampleScroll / 26));
	const samples = $derived(playback.selectedStroke?.points ?? []);
	const filteredPages = $derived(inspector.index?.pages.filter((page) =>
		`${page.name} ${page.id}`.toLowerCase().includes(search.toLowerCase())) ?? []);
	const filteredEntries = $derived(inspector.index?.entries.filter((entry) =>
		entry.name.toLowerCase().includes(search.toLowerCase())) ?? []);

	// Debounce metadata/hex inspection while scrubbing; geometry stays local.
	$effect(() => {
		const offset = playback.selectedOffset;
		const page = inspector.page;
		if (offset === null) return;
		let cancel = () => {};
		const timer = setTimeout(() => {
			if (inspector.page === page && playback.selectedOffset === offset)
				cancel = inspector.inspectSelected();
		}, 120);
		return () => { clearTimeout(timer); cancel(); };
	});
	$effect(() => {
		if (!playback.playing) return;
		let frame = 0, last = performance.now();
		function tick(now: number) {
			const elapsed = now - last;
			last = now;
			playback.advance(elapsed);
			if (playback.playing) frame = requestAnimationFrame(tick);
		}
		frame = requestAnimationFrame(tick);
		return () => cancelAnimationFrame(frame);
	});
	$effect(() => {
		const source = inspector.index?.pages.find((page) => page.visibleIndex === viewerPage);
		untrack(() => {
			if (source && (source.index !== inspector.page || (!playback.replay && !inspector.busy)))
				void inspector.loadPage(source.index, false);
		});
	});
	$effect(() => {
		const visibleIndex = inspector.index?.pages[inspector.page]?.visibleIndex;
		onPreviewChange(playback.replay && visibleIndex != null ? {
			pageIndex: visibleIndex, sourcePage: inspector.page, replay: playback.replay,
			tracks: playback.tracks, position: playback.position,
			selectedOffset: playback.selectedOffset, sampleIndex: playback.sampleIndex,
			onSelect: (offset) => void inspector.pick(offset)
		} : null);
	});
	onMount(() => {
		void inspector.start();
		return () => { inspector.destroy(); onPreviewChange(null); };
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
			value={inspector.page}
			onchange={(e) => void inspector.loadPage(Number(e.currentTarget.value))}
		>
			{#each inspector.index?.pages ?? [] as p}<option value={p.index}
					>{p.index + 1} · {p.id.slice(0, 8)}</option
				>{/each}
		</select></label
	>
	{#if inspector.index?.pages[inspector.page]?.visibleIndex === null}<p class="notice">
			This stored page has no visible preview.
		</p>{/if}
	{#if inspector.error}<p role="alert" class="error">{inspector.error}</p>{/if}
	<div class="timeline">
		<div class="controls">
			<IconButton
				label="Restart replay"
				tooltip
				disabled={!playback.tracks.length}
				onclick={() => playback.scrub(0)}
				><RotateCcw
					size={14}
					strokeWidth={1.5}
					aria-hidden="true"
				/></IconButton
			>
			<IconButton
				label="Previous stroke"
				tooltip
				disabled={!playback.tracks.length}
				onclick={() => inspector.step(-1)}
				><StepBack size={14} strokeWidth={1.5} aria-hidden="true" /></IconButton
			>
			<IconButton
				label={playback.playing ? 'Pause' : 'Play'}
				tooltip
				disabled={!playback.duration}
				onclick={() => {
					if (playback.position >= playback.duration) playback.position = 0;
					playback.playing = !playback.playing;
				}}
			>
				{#if playback.playing}<Pause
						size={14}
						strokeWidth={1.5}
						aria-hidden="true"
					/>{:else}<Play size={14} strokeWidth={1.5} aria-hidden="true" />{/if}
			</IconButton>
			<IconButton
				label="Next stroke"
				tooltip
				disabled={!playback.tracks.length}
				onclick={() => inspector.step(1)}
				><StepForward
					size={14}
					strokeWidth={1.5}
					aria-hidden="true"
				/></IconButton
			>
			<Gauge size={13} aria-hidden="true" />
			<select aria-label="Playback speed" bind:value={playback.speed}
				>{#each [0.25, 0.5, 1, 2, 4] as rate}<option value={rate}
						>{rate}×</option
					>{/each}</select
			>
			<span
				>{(playback.position / 1000).toFixed(2)} / {(playback.duration / 1000).toFixed(2)} s</span
			>
		</div>
		<input
			aria-label="Replay position"
			type="range"
			min="0"
			max={playback.duration || 1}
			step="1"
			value={playback.position}
			disabled={!playback.duration}
			oninput={(e) => playback.scrub(Number(e.currentTarget.value))}
		/>
		<p>
			{playback.tracks.length} strokes · {INTER_STROKE_GAP_MS} ms synthetic gaps{playback.tracks.some(
				(t) => t.synthetic
			)
				? ` · unverified timing uses ${SYNTHETIC_SAMPLE_INTERVAL_MS} ms/sample`
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
				<button onclick={() => void inspector.choose({ kind: 'document' })}
					><FileJson size={14} aria-hidden="true" />Document metadata &
					diagnostics</button
				>
				<button
					onclick={() => inspector.selectOriginal()}><Binary size={14} aria-hidden="true" />Original file bytes</button
				>
				<h3><Files size={13} aria-hidden="true" />Stored pages</h3>
				{#each filteredPages.slice(0, pageCount) as p (p.index)}<RecordTree
						label={`Page ${p.index + 1} · ${p.id.slice(0, 8)}`}
						request={{ kind: 'page', page: p.index }}
						inspect={inspector.choose}
						selected={inspector.selected}
					/>{/each}
				{#if filteredPages.length > pageCount}<button
						onclick={() => (pageCount += 100)}
						><ListPlus size={13} aria-hidden="true" />More pages</button
					>{/if}
				<h3><Archive size={13} aria-hidden="true" />Archive entries</h3>
				{#each filteredEntries.slice(0, entryCount) as e (e.index)}<button
						class="entry"
						onclick={() => void inspector.choose({ kind: 'entry', entry: e.index })}
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
					<ScanSearch size={13} aria-hidden="true" />Selected record {playback.selectedOffset !==
					null
						? `@ 0x${playback.selectedOffset.toString(16)}`
						: ''}
				</h3>
				{#key inspector.selected}<ValueTree value={inspector.properties} />{/key}
				{#if playback.selectedStroke}
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
								value={Math.max(0, playback.sampleIndex)}
								onchange={(event) =>
									playback.selectSample(
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
								>t = {playback.selectedStroke.timestamps[Math.max(0, playback.sampleIndex)] ??
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
										class:active={playback.sampleIndex === n}
										onclick={() => playback.selectSample(n)}
										title={`Sample ${n}`}
									>
										{n}: {point.x.toFixed(2)}, {point.y.toFixed(2)} · {playback.selectedStroke.pressures[
											n
										]?.toFixed(3) ?? '—'} · {playback.selectedStroke.timestamps[n] ??
											'—'} ·
										{playback.selectedStroke.tilts[n]?.toFixed(3) ?? '—'} · {playback.selectedStroke.orientations[
											n
										]?.toFixed(3) ?? '—'}
									</button>
								{/each}
							</div>
						</div>
					</div>
				{/if}
				<h3>
					<Binary size={13} aria-hidden="true" />Hex / ASCII · {inspector.entry === null
						? 'Original file'
						: `Entry ${inspector.entry}`}
				</h3>
				<div class="controls">
					<IconButton
						label="Previous bytes"
						tooltip
						disabled={inspector.byteOffset === 0}
						onclick={() => void inspector.readHex(Math.max(0, inspector.byteOffset - 4096))}
						><ChevronLeft size={14} aria-hidden="true" /></IconButton
					>
					<label
						>Offset <input
							aria-label="Byte offset"
							type="number"
							min="0"
							max={inspector.byteTotal}
							bind:value={inspector.byteOffset}
							onchange={() => void inspector.readHex()}
						/></label
					>
					<IconButton
						label="Next bytes"
						tooltip
						disabled={inspector.byteOffset + 4096 >= inspector.byteTotal}
						onclick={() => void inspector.readHex(inspector.byteOffset + 4096)}
						><ChevronRight size={14} aria-hidden="true" /></IconButton
					>
				</div>
				<pre class="hex">{inspector.hex ||
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
