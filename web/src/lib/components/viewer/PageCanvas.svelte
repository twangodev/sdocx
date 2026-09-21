<script lang="ts">
	import { onMount } from 'svelte';
	import type { DocumentZoomCamera } from '$lib/viewer/document-zoom-camera.svelte';
	import { pageRegion, type PageRegion } from '$lib/viewer/page-region';

	let {
		camera,
		pageWidth,
		pageHeight,
		version,
		render
	}: {
		camera: DocumentZoomCamera;
		pageWidth: number;
		pageHeight: number;
		version: string;
		/** Return false to continue bounded work in the next frame. */
		render: (context: CanvasRenderingContext2D, region: PageRegion) => boolean;
	} = $props();
	let host: HTMLDivElement;
	let canvas: HTMLCanvasElement;
	let frame = 0;
	let mounted = false;
	function schedule() {
		if (mounted && !frame) frame = requestAnimationFrame(paint);
	}
	function paint() {
		frame = 0;
		if (!camera.scroller || camera.gestureZoom !== null) return;
		const page = host.getBoundingClientRect();
		const viewport = camera.scroller.getBoundingClientRect();
		const left = Math.max(0, viewport.left + camera.scroller.clientLeft);
		const top = Math.max(0, viewport.top + camera.scroller.clientTop);
		const clip = {
			left,
			top,
			width: Math.max(
				0,
				Math.min(
					window.innerWidth,
					viewport.left +
						camera.scroller.clientLeft +
						camera.scroller.clientWidth
				) - left
			),
			height: Math.max(
				0,
				Math.min(
					window.innerHeight,
					viewport.top +
						camera.scroller.clientTop +
						camera.scroller.clientHeight
				) - top
			)
		};
		const region = pageRegion(
			page,
			clip,
			pageWidth,
			pageHeight,
			window.devicePixelRatio || 1
		);
		if (!region) {
			canvas.width = canvas.height = 0;
			return;
		}
		if (canvas.width !== region.width) canvas.width = region.width;
		if (canvas.height !== region.height) canvas.height = region.height;
		canvas.style.left = `${(100 * region.x) / (pageWidth * region.scale)}%`;
		canvas.style.top = `${(100 * region.y) / (pageHeight * region.scale)}%`;
		canvas.style.width = `${(100 * region.width) / (pageWidth * region.scale)}%`;
		canvas.style.height = `${(100 * region.height) / (pageHeight * region.scale)}%`;
		const context = canvas.getContext('2d');
		if (!context) return;
		if (!render(context, region)) schedule();
		else {
			canvas.dataset.renderVersion = version;
			canvas.dataset.rasterScale = String(region.scale);
			canvas.dataset.sourceX = String(region.x / region.scale);
			canvas.dataset.sourceY = String(region.y / region.scale);
		}
	}
	$effect(() => {
		version;
		render;
		pageWidth;
		pageHeight;
		camera.surfaceTransform;
		camera.committedZoom;
		camera.pageFit;
		camera.gestureZoom;
		schedule();
	});
	onMount(() => {
		mounted = true;
		const observer = new ResizeObserver(schedule);
		observer.observe(host);
		if (camera.scroller) observer.observe(camera.scroller);
		// Capture scrolls from any ancestor, including the mobile document viewport.
		window.addEventListener('scroll', schedule, true);
		window.addEventListener('resize', schedule);
		let densityQuery: MediaQueryList;
		function watchDensity() {
			densityQuery?.removeEventListener('change', watchDensity);
			densityQuery = window.matchMedia(
				`(resolution: ${window.devicePixelRatio}dppx)`
			);
			densityQuery.addEventListener('change', watchDensity);
			schedule();
		}
		watchDensity();
		const scroller = camera.scroller;
		scroller?.addEventListener('wheel', schedule, { passive: true });
		schedule();
		return () => {
			mounted = false;
			cancelAnimationFrame(frame);
			observer.disconnect();
			window.removeEventListener('scroll', schedule, true);
			window.removeEventListener('resize', schedule);
			densityQuery.removeEventListener('change', watchDensity);
			scroller?.removeEventListener('wheel', schedule);
			canvas.width = canvas.height = 0;
		};
	});
</script>

<div bind:this={host} class="page-canvas" aria-hidden="true">
	<canvas bind:this={canvas} width="0" height="0"></canvas>
</div>

<style>
	.page-canvas {
		position: absolute;
		inset: 0;
		overflow: hidden;
		pointer-events: none;
	}
	canvas {
		position: absolute;
	}
</style>
