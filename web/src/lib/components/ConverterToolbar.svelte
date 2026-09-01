<script lang="ts">
	import { ChevronLeft, ChevronRight, Minus, Plus } from '@lucide/svelte';
	import { separator, type MenuLeaf } from '$lib/menu';
	import { viewerCommandForKey } from '$lib/viewer/keyboard';
	import CompactSelectMenu from './ui/CompactSelectMenu.svelte';
	import IconButton from './ui/IconButton.svelte';
	import PageNumberInput from './ui/PageNumberInput.svelte';

	interface Props {
		pageIndex: number;
		pageCount: number;
		previewZoom: number;
		fitPage: boolean;
		disabled?: boolean;
		onSelectPage: (page: number) => void;
		onStepPage: (direction: -1 | 1) => void;
		onSetZoom: (zoom: number) => void;
		onStepZoom: (direction: -1 | 1) => void;
		onFitWidth: () => void;
		onFitPage: () => void;
		class?: string;
	}

	let {
		pageIndex,
		pageCount,
		previewZoom,
		fitPage,
		disabled = false,
		onSelectPage,
		onStepPage,
		onSetZoom,
		onStepZoom,
		onFitWidth,
		onFitPage,
		class: className = ''
	}: Props = $props();

	const zoomSteps = [50, 75, 100, 125, 150, 175, 200];
	type ZoomAction = 'page' | 'width' | number;

	const zoomMenu = $derived(makeZoomMenu(fitPage, previewZoom));
	const zoomValue = $derived(
		fitPage ? 'Fit page' : previewZoom === 100 ? 'Fit width' : `${previewZoom}%`
	);

	function makeZoomMenu(pageFit: boolean, zoom: number): MenuLeaf<ZoomAction>[] {
		return [
			{ kind: 'action', label: 'Fit page', action: 'page', checked: pageFit },
			{ kind: 'action', label: 'Fit width', action: 'width', checked: !pageFit && zoom === 100 },
			separator(),
			...zoomSteps.filter((step) => step !== 100).map((step) => ({
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
			case 'reset-zoom':
				onFitWidth();
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

<div class="flex h-7 shrink-0 items-center gap-2 {className}">
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

	<span class="h-4 w-px bg-subtle" aria-hidden="true"></span>

	<div class="flex h-6 items-center gap-0.5" role="group" aria-label="Preview zoom">
		<IconButton
			label="Zoom out"
			tooltip="Zoom out · Ctrl/⌘ − · pinch"
			disabled={disabled || (!fitPage && previewZoom === zoomSteps[0])}
			onclick={() => onStepZoom(-1)}
		>
			<Minus size={12} strokeWidth={1.4} />
		</IconButton>
		<CompactSelectMenu
			label="Zoom and page fit"
			value={zoomValue}
			items={zoomMenu}
			onAction={runZoomAction}
			{disabled}
			class="min-w-[5.5rem] justify-center"
		/>
		<IconButton
			label="Zoom in"
			tooltip="Zoom in · Ctrl/⌘ + · pinch"
			disabled={disabled || (!fitPage && previewZoom === zoomSteps.at(-1))}
			onclick={() => onStepZoom(1)}
		>
			<Plus size={12} strokeWidth={1.4} />
		</IconButton>
	</div>
</div>
