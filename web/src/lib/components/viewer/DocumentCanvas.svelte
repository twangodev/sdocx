<script lang="ts" generics="T">
	import type { Snippet } from 'svelte';
	import type { DocumentZoomCamera } from '$lib/viewer/document-zoom-camera.svelte';
	import { wheelZoom } from '$lib/viewer/wheel-zoom';

	interface Props {
		pages: readonly T[];
		pageIndex: number;
		zoom: DocumentZoomCamera;
		disabled?: boolean;
		busy?: boolean;
		page: Snippet<[T, number]>;
		empty?: Snippet;
		onPageChange: (pageIndex: number) => void;
		class?: string;
	}

	let {
		pages,
		pageIndex,
		zoom,
		disabled = false,
		busy = false,
		page,
		empty,
		onPageChange,
		class: className = ''
	}: Props = $props();

	function updatePageFromScroll(event: Event): void {
		const scroller = event.currentTarget as HTMLDivElement;
		const anchor = scroller.getBoundingClientRect().top + scroller.clientHeight * 0.3;
		const pageElements = scroller.querySelectorAll<HTMLElement>('[data-page-index]');
		let nextPage = pageIndex;

		for (const pageElement of pageElements) {
			const bounds = pageElement.getBoundingClientRect();
			const index = Number(pageElement.dataset.pageIndex);
			if (bounds.top <= anchor && bounds.bottom > anchor) {
				nextPage = index;
				break;
			}
			if (bounds.top > anchor) {
				nextPage = index;
				break;
			}
		}

		if (nextPage !== pageIndex) onPageChange(nextPage);
	}
</script>

<div
	bind:this={zoom.scroller}
	use:wheelZoom={{
		disabled,
		onZoom: zoom.updateGesture,
		onEnd: () => void zoom.finishGesture()
	}}
	class="canvas-wrap block min-h-0 flex-1 overflow-auto bg-canvas p-[clamp(0.65rem,1.2vw,1rem)] overscroll-contain max-[720px]:min-h-[420px] {className}"
	aria-busy={busy}
	onscroll={updatePageFromScroll}
>
	{#if pages.length}
		<div
			bind:this={zoom.surface}
			class="page-stack mx-auto my-0 flex min-h-full flex-col items-center gap-3.5 animate-[fade-in_var(--motion-standard)_var(--ease-out)_both] {zoom.gestureZoom !== null || zoom.recentering
				? 'will-change-transform'
				: ''}"
			class:fit-page={zoom.pageFit}
			class:zooming={zoom.gestureZoom !== null}
			class:recentering={zoom.recentering}
			data-zoom={zoom.visiblePageFit ? 'page' : zoom.visibleZoom}
			style:width={zoom.pageFit ? '100%' : `${zoom.committedZoom}%`}
			style:transform={zoom.surfaceTransform}
			style:transform-origin={`${zoom.gestureOrigin.x}px ${zoom.gestureOrigin.y}px`}
		>
			{#each pages as pageItem, index}
				<figure
					class="m-0 flex w-full scroll-mt-2.5 flex-col items-center gap-1"
					class:active={pageIndex === index}
					data-page-index={index}
				>
					{@render page(pageItem, index)}
					<figcaption
						class="font-mono text-[0.58rem] {pageIndex === index ? 'text-text' : 'text-muted'}"
					>
						page {index + 1}
					</figcaption>
				</figure>
			{/each}
		</div>
	{:else if empty}
		{@render empty()}
	{/if}
</div>
