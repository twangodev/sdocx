<script lang="ts">
	import { ChevronLeft, ChevronRight, Minus, Plus } from '@lucide/svelte';
	import { separator, type MenuLeaf } from '$lib/menu';
	import { viewerCommandForKey } from '$lib/viewer/keyboard';
	import { formatZoom, MAX_ZOOM, MIN_ZOOM, ZOOM_STEPS } from '$lib/viewer/zoom';
	import CompactSelectMenu from './ui/CompactSelectMenu.svelte';
	import IconButton from './ui/IconButton.svelte';
	import PageNumberInput from './ui/PageNumberInput.svelte';

	interface ConverterToolbarModel {
		pageIndex: number;
		pageCount: number;
		previewZoom: number;
		fitPage: boolean;
		disabled: boolean;
	}

	interface ConverterToolbarActions {
		onSelectPage: (page: number) => void;
		onStepPage: (direction: -1 | 1) => void;
		onSetZoom: (zoom: number) => void;
		onStepZoom: (direction: -1 | 1) => void;
		onFitWidth: () => void;
		onFitPage: () => void;
	}

	interface Props {
		model: ConverterToolbarModel;
		actions: ConverterToolbarActions;
		class?: string;
	}

	let {
		model,
		actions,
		class: className = ''
	}: Props = $props();

	type ZoomAction = 'page' | 'width' | number;

	const zoomMenu = $derived(makeZoomMenu(model.fitPage, model.previewZoom));
	const zoomValue = $derived(
		model.fitPage
			? 'Fit page'
			: model.previewZoom === 100
				? 'Fit width'
				: formatZoom(model.previewZoom)
	);

	function makeZoomMenu(pageFit: boolean, zoom: number): MenuLeaf<ZoomAction>[] {
		return [
			{ kind: 'action', label: 'Fit page', action: 'page', checked: pageFit },
			{ kind: 'action', label: 'Fit width', action: 'width', checked: !pageFit && zoom === 100 },
			separator(),
			...ZOOM_STEPS.filter((step) => step !== 100).map((step) => ({
				kind: 'action' as const,
				label: `${step}%`,
				action: step,
				checked: !pageFit && zoom === step
			}))
		];
	}

	function runZoomAction(action: ZoomAction): void {
		if (action === 'page') actions.onFitPage();
		else if (action === 'width') actions.onFitWidth();
		else actions.onSetZoom(action);
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
		if (model.disabled || event.defaultPrevented || event.isComposing || isTextEntry(event.target)) return;

		const command = viewerCommandForKey(event);
		if (!command) return;

		event.preventDefault();
		switch (command) {
			case 'zoom-in':
				actions.onStepZoom(1);
				break;
			case 'zoom-out':
				actions.onStepZoom(-1);
				break;
			case 'reset-zoom':
				actions.onFitWidth();
				break;
			case 'previous-page':
				actions.onStepPage(-1);
				break;
			case 'next-page':
				actions.onStepPage(1);
				break;
			case 'first-page':
				actions.onSelectPage(0);
				break;
			case 'last-page':
				actions.onSelectPage(model.pageCount - 1);
		}
	}
</script>

<svelte:window onkeydown={handleKeyboardShortcut} />

<div class="flex h-7 shrink-0 items-center gap-2 {className}">
	<div class="flex h-6 items-center gap-0.5" role="group" aria-label="Page navigation">
		<IconButton
			label="Previous page"
			tooltip="Previous page · Page Up"
			disabled={model.disabled || model.pageIndex === 0}
			onclick={() => actions.onStepPage(-1)}
		>
			<ChevronLeft size={12} strokeWidth={1.4} />
		</IconButton>
		<PageNumberInput
			pageIndex={model.pageIndex}
			pageCount={model.pageCount}
			onSelect={actions.onSelectPage}
			disabled={model.disabled}
		/>
		<IconButton
			label="Next page"
			tooltip="Next page · Page Down"
			disabled={model.disabled || model.pageIndex === model.pageCount - 1}
			onclick={() => actions.onStepPage(1)}
		>
			<ChevronRight size={12} strokeWidth={1.4} />
		</IconButton>
	</div>

	<span class="h-4 w-px bg-subtle" aria-hidden="true"></span>

	<div class="flex h-6 items-center gap-0.5" role="group" aria-label="Preview zoom">
		<IconButton
			label="Zoom out"
			tooltip="Zoom out · Ctrl/⌘ − · pinch"
			disabled={model.disabled || (!model.fitPage && model.previewZoom <= MIN_ZOOM)}
			onclick={() => actions.onStepZoom(-1)}
		>
			<Minus size={12} strokeWidth={1.4} />
		</IconButton>
		<CompactSelectMenu
			label="Zoom and page fit"
			value={zoomValue}
			items={zoomMenu}
			onAction={runZoomAction}
			disabled={model.disabled}
			class="min-w-[5.5rem] justify-center"
		/>
		<IconButton
			label="Zoom in"
			tooltip="Zoom in · Ctrl/⌘ + · pinch"
			disabled={model.disabled || (!model.fitPage && model.previewZoom >= MAX_ZOOM)}
			onclick={() => actions.onStepZoom(1)}
		>
			<Plus size={12} strokeWidth={1.4} />
		</IconButton>
	</div>
</div>
