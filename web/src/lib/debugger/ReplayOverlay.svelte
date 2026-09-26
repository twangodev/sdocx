<script lang="ts">
	import { untrack } from 'svelte';
	import type { DocumentSession } from '$converter/document-session.svelte';
	import type { Replay, Track } from './model';
	import { sampleAt } from './model';
	import { ReplaySvg } from './replay-svg';
	let {
		replay,
		tracks,
		position,
		selectedOffset,
		sampleIndex,
		onselect,
		sourcePage,
		session
	}: {
		replay: Replay;
		tracks: Track[];
		position: number;
		selectedOffset: number | null;
		sampleIndex: number;
		onselect: (offset: number) => void;
		sourcePage: number;
		session: DocumentSession;
	} = $props();
	let host = $state<HTMLDivElement>();
	let vector = $state.raw<ReplaySvg>();
	let root = $state.raw<SVGSVGElement>();
	let requested = $state(false);
	let error = $state('');
	const partial = $derived(position < (tracks.at(-1)?.end ?? 0));
	const vectorKey = $derived(requested ? `${sourcePage}:${session.colorMode}` : '');
	$effect(() => {
		if (partial) requested = true;
	});
	$effect(() => {
		const key = vectorKey;
		const target = host;
		if (!key || !target) return;
		return untrack(() => {
			const pageReplay = replay;
			const page = sourcePage;
			const colorMode = session.colorMode;
			let cancelled = false;
			vector = undefined;
			root = undefined;
			error = '';
			target.replaceChildren();
			void session.debug({ kind: 'replay-svg', page, colorMode }).then(value => {
				if (cancelled) return;
				const { svg } = value as { svg: string };
				const parsed = new DOMParser().parseFromString(svg, 'image/svg+xml');
				if (parsed.querySelector('parsererror')) throw new Error('Invalid replay SVG');
				const element = document.importNode(parsed.documentElement, true) as unknown as SVGSVGElement;
				const controller = new ReplaySvg(element, pageReplay);
				element.style.width = element.style.height = '100%';
				target.replaceChildren(element);
				root = element;
				vector = controller;
			}).catch(cause => { if (!cancelled) error = String(cause); });
			return () => { cancelled = true; target.replaceChildren(); };
		});
	});
	$effect(() => {
		if (!vector || !root) return;
		let complete = -1;
		while (complete + 1 < tracks.length && tracks[complete + 1].end <= position) complete++;
		const active = complete + 1;
		const sample = active < tracks.length && tracks[active].start <= position
			? sampleAt(tracks[active].times, position - tracks[active].start) : -1;
		vector.seek(complete, sample);
		root.dataset.renderVersion = String(position);
	});
	const selectedStroke = $derived(
		replay.strokes.find((s) => s.offset === selectedOffset)?.stroke
	);
	const selectedBox = $derived(
		selectedStroke?.bbox ??
			replay.objects.find((o) => o.offset === selectedOffset)?.bbox
	);
	const selectedPoint = $derived(selectedStroke?.points[sampleIndex]);
	function pick(event: MouseEvent) {
		const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
		const x = ((event.clientX - rect.left) * replay.width) / rect.width,
			y = ((event.clientY - rect.top) * replay.height) / rect.height;
		const hit = [...replay.objects]
			.reverse()
			.find(
				(o) =>
					o.bbox &&
					x >= o.bbox.x_min - 3 &&
					x <= o.bbox.x_max + 3 &&
					y >= o.bbox.y_min - 3 &&
					y <= o.bbox.y_max + 3
			);
		if (hit) onselect(hit.offset);
	}
</script>

<div
	class="replay-overlay"
	data-replay-overlay
	role="button"
	tabindex="0"
	onclick={pick}
	onkeydown={(event) => {
		if (event.key === 'Enter' && selectedOffset !== null)
			onselect(selectedOffset);
	}}
	aria-label="Inspect page objects. Select from the tree or click the preview."
>
	<div class="vector-page" bind:this={host} hidden={!partial} aria-hidden="true"></div>
	{#if error}<p role="alert">{error}</p>{/if}
	<svg viewBox={`0 0 ${replay.width} ${replay.height}`} aria-hidden="true">
		{#if selectedBox}<rect
				x={selectedBox.x_min}
				y={selectedBox.y_min}
				width={Math.max(1, selectedBox.x_max - selectedBox.x_min)}
				height={Math.max(1, selectedBox.y_max - selectedBox.y_min)}
				fill="none"
				stroke="#2684ff"
				stroke-width="2"
				stroke-dasharray="5 3"
				vector-effect="non-scaling-stroke"
			/>{/if}
		{#if selectedPoint}<circle
				cx={selectedPoint.x}
				cy={selectedPoint.y}
				r={4}
				fill="#e25139"
			/>{/if}
	</svg>
</div>

<style>
	.replay-overlay {
		position: absolute;
		inset: 0;
		cursor: crosshair;
	}
	.vector-page,
	.replay-overlay > svg {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
	}
	.vector-page { pointer-events: none; overflow: hidden; }
	.replay-overlay > svg { pointer-events: none; }
	.replay-overlay p {
		position: absolute;
		z-index: 1;
		background: var(--color-bg);
		color: var(--color-negative);
		font-size: 11px;
	}
</style>
