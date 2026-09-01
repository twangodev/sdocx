<script lang="ts">
	import { Info, RefreshCw, X } from '@lucide/svelte';
	import type { ColorMode } from '$converter/protocol';
	import ConverterToolbar from './ConverterToolbar.svelte';
	import ColorModeSwitch from './ui/ColorModeSwitch.svelte';
	import ExportMenu from './ui/ExportMenu.svelte';
	import IconButton from './ui/IconButton.svelte';

	type Scale = 1 | 2;
	type ArchiveKind = 'svg' | 'png' | 'everything';

	interface Props {
		title: string;
		filename: string;
		fileSize: number;
		pageIndex: number;
		pageCount: number;
		previewZoom: number;
		fitPage: boolean;
		colorMode: ColorMode;
		detailsOpen: boolean;
		exporting: boolean;
		rendering: boolean;
		pngScale: Scale;
		exportProgress: string;
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

	let {
		title,
		filename,
		fileSize,
		pageIndex,
		pageCount,
		previewZoom,
		fitPage,
		colorMode,
		detailsOpen,
		exporting,
		rendering,
		pngScale,
		exportProgress,
		onToggleDetails,
		onSelectPage,
		onStepPage,
		onSetZoom,
		onStepZoom,
		onFitWidth,
		onFitPage,
		onColorMode,
		onScale,
		onCurrentSvg,
		onCurrentPng,
		onArchive,
		onJson,
		onCancel,
		onReplace,
		onClose
	}: Props = $props();

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
			active={detailsOpen}
			onclick={onToggleDetails}
		>
			<Info size={13} strokeWidth={1.4} />
		</IconButton>
		<div class="min-w-0 leading-tight">
			<h1 class="block truncate text-[11px] font-[550]" title={title}>{title}</h1>
			<span class="block truncate font-mono text-[9px] text-muted" title={filename}>
				{formatBytes(fileSize)} · {pageCount} {pageCount === 1 ? 'page' : 'pages'} · local
			</span>
		</div>
	</div>

	<ConverterToolbar
		{pageIndex}
		{pageCount}
		{previewZoom}
		{fitPage}
		disabled={exporting || rendering}
		onSelectPage={onSelectPage}
		onStepPage={onStepPage}
		onSetZoom={onSetZoom}
		onStepZoom={onStepZoom}
		onFitWidth={onFitWidth}
		onFitPage={onFitPage}
		class="justify-self-center max-[820px]:order-3 max-[820px]:col-span-2"
	/>

	<div class="flex min-w-0 items-center justify-end gap-1">
		{#if exporting}
			<span class="motion-fade-in max-w-36 truncate font-mono text-[9px] text-muted max-[620px]:hidden">
				{exportProgress}
			</span>
		{/if}
		<ColorModeSwitch value={colorMode} disabled={rendering} onChange={onColorMode} />
		<span class="mx-0.5 h-4 w-px bg-subtle" aria-hidden="true"></span>
		<ExportMenu
			{exporting}
			{rendering}
			{pngScale}
			onScale={onScale}
			onCurrentSvg={onCurrentSvg}
			onCurrentPng={onCurrentPng}
			onArchive={onArchive}
			onJson={onJson}
			onCancel={onCancel}
		/>
		<IconButton label="Replace document" tooltip onclick={onReplace}>
			<RefreshCw size={12} strokeWidth={1.4} />
		</IconButton>
		<IconButton label="Close document" tooltip onclick={onClose}>
			<X size={13} strokeWidth={1.4} />
		</IconButton>
	</div>
</header>
