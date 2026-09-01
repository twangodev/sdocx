<script lang="ts">
	import {
		ChevronLeft,
		ChevronRight,
		Maximize2,
		Minus,
		Monitor,
		Moon,
		MoveHorizontal,
		Plus,
		Sun
	} from '@lucide/svelte';
	import type { ColorMode } from '$converter/protocol';
	import { separator, type MenuLeaf } from '$lib/menu';
	import CompactSelectMenu from './ui/CompactSelectMenu.svelte';
	import IconButton from './ui/IconButton.svelte';
	import PageNumberInput from './ui/PageNumberInput.svelte';
	import SegmentedControl from './ui/SegmentedControl.svelte';

	interface Props {
		pageIndex: number;
		pageCount: number;
		previewZoom: number;
		fitPage: boolean;
		colorMode: ColorMode;
		disabled?: boolean;
		onSelectPage: (page: number) => void;
		onStepPage: (direction: -1 | 1) => void;
		onSetZoom: (zoom: number) => void;
		onStepZoom: (direction: -1 | 1) => void;
		onFitWidth: () => void;
		onFitPage: () => void;
		onColorMode: (mode: ColorMode) => void;
	}

	let {
		pageIndex,
		pageCount,
		previewZoom,
		fitPage,
		colorMode,
		disabled = false,
		onSelectPage,
		onStepPage,
		onSetZoom,
		onStepZoom,
		onFitWidth,
		onFitPage,
		onColorMode
	}: Props = $props();

	const zoomSteps = [50, 75, 100, 125, 150, 175, 200];
	const colorModes = ['auto', 'light', 'dark'] as const;
	type ZoomAction = 'page' | 'width' | number;

	const zoomMenu = $derived(makeZoomMenu(fitPage, previewZoom));

	function makeZoomMenu(pageFit: boolean, zoom: number): MenuLeaf<ZoomAction>[] {
		return [
			{ kind: 'action', label: 'fit page', action: 'page', checked: pageFit },
			{ kind: 'action', label: 'fit width', action: 'width', checked: !pageFit && zoom === 100 },
			separator(),
			...zoomSteps.map((step) => ({
				kind: 'action' as const,
				label: `${step}%`,
				action: step,
				checked: !pageFit && zoom === step
			}))
		];
	}

	function runZoomAction(action: ZoomAction): void {
		if (action === 'page') onFitPage();
		else if (action === 'width') onFitWidth();
		else onSetZoom(action);
	}

	function colorModeLabel(mode: ColorMode): string {
		if (mode === 'auto') return 'Automatic color mode';
		if (mode === 'light') return 'Light document mode';
		return 'Dark document mode';
	}
</script>

<div
	class="preview-toolbar flex h-9 shrink-0 items-center gap-1 border-b border-subtle bg-bg px-2 max-[520px]:h-auto max-[520px]:items-stretch max-[520px]:flex-col max-[520px]:py-2"
>
	<div class="flex h-6 items-center gap-0.5" role="group" aria-label="Page navigation">
		<IconButton
			label="Previous page"
			disabled={disabled || pageIndex === 0}
			onclick={() => onStepPage(-1)}
		>
			<ChevronLeft size={12} strokeWidth={1.4} />
		</IconButton>
		<PageNumberInput
			{pageIndex}
			{pageCount}
			onSelect={onSelectPage}
			{disabled}
		/>
		<IconButton
			label="Next page"
			disabled={disabled || pageIndex === pageCount - 1}
			onclick={() => onStepPage(1)}
		>
			<ChevronRight size={12} strokeWidth={1.4} />
		</IconButton>
	</div>

	<div class="flex h-6 items-center gap-0.5" role="group" aria-label="Preview zoom">
		<IconButton label="Fit page" tooltip active={fitPage} {disabled} onclick={onFitPage}>
			<Maximize2 size={12} strokeWidth={1.4} />
		</IconButton>
		<IconButton
			label="Fit width"
			tooltip
			active={!fitPage && previewZoom === 100}
			{disabled}
			onclick={onFitWidth}
		>
			<MoveHorizontal size={12} strokeWidth={1.4} />
		</IconButton>
		<IconButton
			label="Zoom out"
			disabled={disabled || (!fitPage && previewZoom === zoomSteps[0])}
			onclick={() => onStepZoom(-1)}
		>
			<Minus size={12} strokeWidth={1.4} />
		</IconButton>
		<CompactSelectMenu
			label="Choose zoom level"
			value={fitPage ? 'fit' : `${previewZoom}%`}
			items={zoomMenu}
			onAction={runZoomAction}
			{disabled}
			chevron={false}
			class="min-w-12 justify-center"
		/>
		<IconButton
			label="Zoom in"
			disabled={disabled || (!fitPage && previewZoom === zoomSteps.at(-1))}
			onclick={() => onStepZoom(1)}
		>
			<Plus size={12} strokeWidth={1.4} />
		</IconButton>
	</div>

	<SegmentedControl
		options={colorModes}
		label="Document color mode"
		{disabled}
		itemLabel={colorModeLabel}
		itemTitle={colorModeLabel}
		bind:value={() => colorMode, (next) => onColorMode(next)}
		class="ml-auto w-24 max-[520px]:ml-0 max-[520px]:w-full"
	>
		{#snippet item(mode)}
			{#if mode === 'auto'}
				<Monitor size={12} strokeWidth={1.4} />
			{:else if mode === 'light'}
				<Sun size={12} strokeWidth={1.4} />
			{:else}
				<Moon size={12} strokeWidth={1.4} />
			{/if}
		{/snippet}
	</SegmentedControl>
</div>
