<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import type { DocumentSession } from '$converter/document-session.svelte';
	import PageCanvas from '$lib/components/viewer/PageCanvas.svelte';
	import type { PageRegion } from '$lib/viewer/page-region';
	import type { DocumentZoomCamera } from '$lib/viewer/document-zoom-camera.svelte';
	import type { Replay, Track } from './model';
	import { sampleAt } from './model';
	import { ReplayRaster } from './replay-raster';
	let {
		replay,
		tracks,
		position,
		selectedOffset,
		sampleIndex,
		onselect,
		sourcePage,
		session,
		camera
	}: {
		replay: Replay;
		tracks: Track[];
		position: number;
		selectedOffset: number | null;
		sampleIndex: number;
		onselect: (offset: number) => void;
		sourcePage: number;
		session: DocumentSession;
		camera: DocumentZoomCamera;
	} = $props();
	let background = $state('');
	let requested = $state(false);
	$effect(() => {
		if (partial) requested = true;
	});
	let raster: ReplayRaster | undefined;
	let lastReplay: Replay | undefined;
	const partial = $derived(position < (tracks.at(-1)?.end ?? 0));
	let defaultInk = $state('#1a1a1a');
	let backgroundError = $state('');
	let lastInk = '';
	const backgroundKey = $derived(
		requested ? `${sourcePage}:${session.colorMode}` : ''
	);
	$effect(() => {
		const key = backgroundKey;
		return untrack(() => {
			const request = {
				kind: 'background' as const,
				page: sourcePage,
				colorMode: session.colorMode
			};
			background = '';
			backgroundError = '';
			if (!key) return;
			let cancelled = false,
				url = '';
			void session
				.debug(request)
				.then((value) => {
					if (cancelled) return;
					const result = value as { svg: string; defaultInk: string };
					url = URL.createObjectURL(
						new Blob([result.svg], { type: 'image/svg+xml' })
					);
					background = url;
					defaultInk = result.defaultInk;
				})
				.catch((error) => {
					if (!cancelled) backgroundError = String(error);
				});
			return () => {
				cancelled = true;
				if (url) URL.revokeObjectURL(url);
			};
		});
	});
	function paint(ctx: CanvasRenderingContext2D, region: PageRegion): boolean {
		if (!raster || lastReplay !== replay || lastInk !== defaultInk) {
			raster?.dispose();
			raster = new ReplayRaster(replay, defaultInk);
			lastReplay = replay;
			lastInk = defaultInk;
		}
		let complete = -1;
		while (complete + 1 < tracks.length && tracks[complete + 1].end <= position)
			complete++;
		const active = complete + 1;
		const sample =
			active < tracks.length && tracks[active].start <= position
				? sampleAt(tracks[active].times, position - tracks[active].start)
				: -1;
		if (!raster.paint(ctx, region, complete, sample)) return false;
		const { scale, pixelRatio } = region;
		ctx.setTransform(scale, 0, 0, scale, -region.x, -region.y);
		if (selectedBox) {
			ctx.strokeStyle = '#2684ff';
			ctx.lineWidth = (2 * pixelRatio) / scale;
			ctx.setLineDash([(5 * pixelRatio) / scale, (3 * pixelRatio) / scale]);
			ctx.strokeRect(
				selectedBox.x_min,
				selectedBox.y_min,
				Math.max(1, selectedBox.x_max - selectedBox.x_min),
				Math.max(1, selectedBox.y_max - selectedBox.y_min)
			);
			ctx.setLineDash([]);
		}
		if (selectedPoint) {
			ctx.fillStyle = '#e25139';
			ctx.beginPath();
			ctx.arc(
				selectedPoint.x,
				selectedPoint.y,
				(4 * pixelRatio) / scale,
				0,
				Math.PI * 2
			);
			ctx.fill();
		}
		return true;
	}
	const selectedStroke = $derived(
		replay.strokes.find((s) => s.offset === selectedOffset)?.stroke
	);
	const selectedBox = $derived(
		selectedStroke?.bbox ??
			replay.objects.find((o) => o.offset === selectedOffset)?.bbox
	);
	const selectedPoint = $derived(selectedStroke?.points[sampleIndex]);
	$effect(() => {
		const page = replay,
			ink = defaultInk;
		if (lastReplay !== page || lastInk !== ink) {
			raster?.dispose();
			raster = undefined;
		}
	});
	onMount(() => () => raster?.dispose());
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
	{#if partial && background}
		<img src={background} alt="Replay page background" />
		<PageCanvas
			{camera}
			pageWidth={replay.width}
			pageHeight={replay.height}
			version={`${position}:${selectedOffset}:${sampleIndex}:${defaultInk}`}
			render={paint}
		/>
	{/if}
	{#if backgroundError}<p role="alert">{backgroundError}</p>{/if}
	{#if !partial}
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
	{/if}
</div>

<style>
	.replay-overlay {
		position: absolute;
		inset: 0;
		cursor: crosshair;
	}
	.replay-overlay img,
	.replay-overlay svg {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
	}
	.replay-overlay p {
		position: absolute;
		z-index: 1;
		background: var(--color-bg);
		color: var(--color-negative);
		font-size: 11px;
	}
</style>
