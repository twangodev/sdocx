<script lang="ts">
	import {
		ChevronLeft,
		ChevronRight,
		Maximize2,
		Minus,
		MoveHorizontal,
		Plus
	} from '@lucide/svelte';
	import type { ColorMode } from '$converter/protocol';
	import { separator, type MenuLeaf } from '$lib/menu';
	import { viewerCommandForKey } from '$lib/viewer/keyboard';
	import CompactSelectMenu from './ui/CompactSelectMenu.svelte';
	import ColorModeSwitch from './ui/ColorModeSwitch.svelte';
	import IconButton from './ui/IconButton.svelte';
	import PageNumberInput from './ui/PageNumberInput.svelte';

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

	function isTextEntry(target: EventTarget | null): boolean {
		return (
			target instanceof HTMLElement &&
			(target.isContentEditable ||
				['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName) ||
				Boolean(target.closest('[contenteditable="true"]')))
		);
	}

	function handleKeyboardShortcut(event: KeyboardEvent): void {
		if (disabled || event.defaultPrevented || event.isComposing || isTextEntry(event.target)) return;

		const command = viewerCommandForKey(event);
		if (!command) return;

		event.preventDefault();
		switch (command) {
			case 'zoom-in':
				onStepZoom(1);
				break;
			case 'zoom-out':
				onStepZoom(-1);
				break;
			case 'previous-page':
				onStepPage(-1);
				break;
			case 'next-page':
				onStepPage(1);
				break;
			case 'first-page':
				onSelectPage(0);
				break;
			case 'last-page':
				onSelectPage(pageCount - 1);
		}
	}
</script>

<svelte:window onkeydown={handleKeyboardShortcut} />

<div
	class="preview-toolbar flex h-9 shrink-0 items-center gap-1 border-b border-subtle bg-bg px-2 max-[520px]:h-auto max-[520px]:items-stretch max-[520px]:flex-col max-[520px]:py-2"
>
	<div class="flex h-6 items-center gap-0.5" role="group" aria-label="Page navigation">
		<IconButton
			label="Previous page"
			tooltip="Previous page · Page Up"
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
			tooltip="Next page · Page Down"
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
			tooltip="Zoom out · Ctrl/⌘ −"
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
			tooltip="Zoom in · Ctrl/⌘ +"
			disabled={disabled || (!fitPage && previewZoom === zoomSteps.at(-1))}
			onclick={() => onStepZoom(1)}
		>
			<Plus size={12} strokeWidth={1.4} />
		</IconButton>
	</div>

	<ColorModeSwitch
		value={colorMode}
		{disabled}
		onChange={onColorMode}
		class="ml-auto max-[520px]:ml-0 max-[520px]:self-end"
	/>
</div>
