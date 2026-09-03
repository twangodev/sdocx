<script lang="ts">
	import type { CommitPageDiff } from '$lib/regression/commit-compare';

	interface Props {
		page: CommitPageDiff;
		aspectRatio: string;
		leftLabel: string;
		rightLabel: string;
	}

	let { page, aspectRatio, leftLabel, rightLabel }: Props = $props();
	let swipe = $state(50);
	let dragging = false;
	let comparisonElement = $state<HTMLDivElement>();

	function setSwipe(event: PointerEvent): void {
		if (!comparisonElement) return;
		const bounds = comparisonElement.getBoundingClientRect();
		swipe = Math.min(100, Math.max(0, ((event.clientX - bounds.left) / bounds.width) * 100));
	}

	function startSwipe(event: PointerEvent): void {
		dragging = true;
		comparisonElement?.setPointerCapture(event.pointerId);
		setSwipe(event);
	}

	function moveSwipe(event: PointerEvent): void {
		if (dragging) setSwipe(event);
	}

	function moveSwipeWithKeyboard(event: KeyboardEvent): void {
		if (event.key === 'ArrowLeft') swipe = Math.max(0, swipe - 2);
		else if (event.key === 'ArrowRight') swipe = Math.min(100, swipe + 2);
		else return;
		event.preventDefault();
	}
</script>

<div
	bind:this={comparisonElement}
	class="relative mx-auto w-full max-w-[72rem] cursor-ew-resize touch-none overflow-hidden border border-black/15 bg-white shadow-[0_6px_20px_rgb(0_0_0_/_0.12)] select-none"
	style:aspect-ratio={aspectRatio}
	role="slider"
	tabindex="0"
	aria-label="Swipe between commit renders"
	aria-valuemin="0"
	aria-valuemax="100"
	aria-valuenow={Math.round(swipe)}
	onpointerdown={startSwipe}
	onpointermove={moveSwipe}
	onpointerup={() => (dragging = false)}
	onpointercancel={() => (dragging = false)}
	onkeydown={moveSwipeWithKeyboard}
>
	{#if page.leftUrl}<img class="absolute inset-0 size-full" src={page.leftUrl} alt="" />{/if}
	{#if page.rightUrl}
		<img
			class="absolute inset-0 size-full"
			src={page.rightUrl}
			alt=""
			style:clip-path={`inset(0 0 0 ${swipe}%)`}
		/>
	{/if}
	<span
		class="pointer-events-none absolute inset-y-0 w-px -translate-x-1/2 bg-accent shadow-[0_0_0_1px_white]"
		style:left={`${swipe}%`}
	></span>
	<span class="pointer-events-none absolute top-2 left-2 rounded bg-black/65 px-1.5 py-1 font-mono text-[10px] text-white">{leftLabel}</span>
	<span class="pointer-events-none absolute top-2 right-2 rounded bg-black/65 px-1.5 py-1 font-mono text-[10px] text-white">{rightLabel}</span>
</div>
