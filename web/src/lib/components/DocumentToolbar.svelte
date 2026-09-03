<script lang="ts">
	import { Info, RefreshCw, X } from '@lucide/svelte';
	import type { ColorMode } from '$converter/protocol';
	import ConverterToolbar from './ConverterToolbar.svelte';
	import ColorModeSwitch from './ui/ColorModeSwitch.svelte';
	import ExportMenu from './ui/ExportMenu.svelte';
	import IconButton from './ui/IconButton.svelte';

	type Scale = 1 | 2;
	type ArchiveKind = 'svg' | 'png' | 'everything';

	interface DocumentToolbarModel {
		document: {
			title: string;
			filename: string;
			fileSize: number;
			pageCount: number;
		};
		viewer: {
			pageIndex: number;
			previewZoom: number;
			fitPage: boolean;
			colorMode: ColorMode;
			detailsOpen: boolean;
		};
		activity: {
			exporting: boolean;
			rendering: boolean;
			pngScale: Scale;
			exportProgress: string;
		};
	}

	interface DocumentToolbarActions {
		onToggleDetails: () => void;
		onSelectPage: (page: number) => void;
		onStepPage: (direction: -1 | 1) => void;
		onSetZoom: (zoom: number) => void;
		onStepZoom: (direction: -1 | 1) => void;
		onFitWidth: () => void;
		onFitPage: () => void;
		onColorMode: (mode: ColorMode) => void;
		onScale: (scale: Scale) => void;
		onCurrentSvg: () => void;
		onCurrentPng: () => void;
		onArchive: (kind: ArchiveKind) => void;
		onJson: () => void;
		onCancel: () => void;
		onReplace: () => void;
		onClose: () => void;
	}

	interface Props {
		model: DocumentToolbarModel;
		actions: DocumentToolbarActions;
	}

	let { model, actions }: Props = $props();

	function formatBytes(bytes: number): string {
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
		return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
	}
</script>

<header
	class="grid min-h-11 shrink-0 grid-cols-[minmax(10rem,1fr)_auto_minmax(10rem,1fr)] items-center gap-2 border-b border-subtle bg-bg px-2 py-1 max-[820px]:grid-cols-[minmax(0,1fr)_auto]"
	aria-label="Document toolbar"
>
	<div class="flex min-w-0 items-center gap-1.5">
		<IconButton
			label="Document information"
			tooltip
			active={model.viewer.detailsOpen}
			onclick={actions.onToggleDetails}
		>
			<Info size={13} strokeWidth={1.4} />
		</IconButton>
		<div class="min-w-0 leading-tight">
			<h1 class="block truncate text-[11px] font-[550]" title={model.document.title}>{model.document.title}</h1>
			<span class="block truncate font-mono text-[9px] text-muted" title={model.document.filename}>
				{formatBytes(model.document.fileSize)} · {model.document.pageCount} {model.document.pageCount === 1 ? 'page' : 'pages'} · local
			</span>
		</div>
	</div>

	<ConverterToolbar
		model={{
			pageIndex: model.viewer.pageIndex,
			pageCount: model.document.pageCount,
			previewZoom: model.viewer.previewZoom,
			fitPage: model.viewer.fitPage,
			disabled: model.activity.exporting || model.activity.rendering
		}}
		actions={{
			onSelectPage: actions.onSelectPage,
			onStepPage: actions.onStepPage,
			onSetZoom: actions.onSetZoom,
			onStepZoom: actions.onStepZoom,
			onFitWidth: actions.onFitWidth,
			onFitPage: actions.onFitPage
		}}
		class="justify-self-center max-[820px]:order-3 max-[820px]:col-span-2"
	/>

	<div class="flex min-w-0 items-center justify-end gap-1">
		{#if model.activity.exporting}
			<span class="motion-fade-in max-w-36 truncate font-mono text-[9px] text-muted max-[620px]:hidden">
				{model.activity.exportProgress}
			</span>
		{/if}
		<ColorModeSwitch value={model.viewer.colorMode} disabled={model.activity.rendering} onChange={actions.onColorMode} />
		<span class="mx-0.5 h-4 w-px bg-subtle" aria-hidden="true"></span>
		<ExportMenu
			model={{
				exporting: model.activity.exporting,
				rendering: model.activity.rendering,
				pngScale: model.activity.pngScale
			}}
			actions={{
				onScale: actions.onScale,
				onCurrentSvg: actions.onCurrentSvg,
				onCurrentPng: actions.onCurrentPng,
				onArchive: actions.onArchive,
				onJson: actions.onJson,
				onCancel: actions.onCancel
			}}
		/>
		<IconButton label="Replace document" tooltip onclick={actions.onReplace}>
			<RefreshCw size={12} strokeWidth={1.4} />
		</IconButton>
		<IconButton label="Close document" tooltip onclick={actions.onClose}>
			<X size={13} strokeWidth={1.4} />
		</IconButton>
	</div>
</header>
