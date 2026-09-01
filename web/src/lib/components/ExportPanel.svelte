<script lang="ts">
	import { Download } from '@lucide/svelte';
	import { buttonClass } from '$lib/button';
	import type { MenuLeaf } from '$lib/menu';
	import CompactSelectMenu from './ui/CompactSelectMenu.svelte';

	type Scale = 1 | 2;
	type ArchiveKind = 'svg' | 'png' | 'everything';

	interface Props {
		exporting: boolean;
		rendering: boolean;
		pngScale: Scale;
		onScale: (scale: Scale) => void;
		onCurrentSvg: () => void;
		onCurrentPng: () => void;
		onArchive: (kind: ArchiveKind) => void;
		onJson: () => void;
		onCancel: () => void;
	}

	let {
		exporting,
		rendering,
		pngScale,
		onScale,
		onCurrentSvg,
		onCurrentPng,
		onArchive,
		onJson,
		onCancel
	}: Props = $props();

	const scaleMenu = $derived(makeScaleMenu(pngScale));
	const primary = buttonClass('primary');
	const secondary = buttonClass('secondary');

	function makeScaleMenu(scale: Scale): MenuLeaf<Scale>[] {
		return ([1, 2] as const).map((value) => ({
			kind: 'action',
			label: `${value}×`,
			action: value,
			checked: value === scale
		}));
	}
</script>

<aside
	class="min-w-0 overflow-auto border-l border-subtle bg-bg p-2.5 max-[1050px]:col-span-full max-[1050px]:grid max-[1050px]:grid-cols-3 max-[1050px]:gap-x-3 max-[1050px]:border-t max-[1050px]:border-l-0 max-[720px]:flex max-[720px]:flex-col"
	aria-label="Export options"
>
	<span class="text-[10px] font-semibold text-muted max-[1050px]:col-span-full">export</span>

	<div class="mt-2 flex flex-col gap-1.5 border-b border-subtle pb-2.5">
		<span class="mb-0.5 text-[10px] text-muted">Current page</span>
		<button class="flex w-full items-center justify-between {primary}" disabled={exporting || rendering} onclick={onCurrentSvg}>
			SVG <Download size={12} strokeWidth={1.4} />
		</button>
		<button class="flex w-full items-center justify-between {secondary}" disabled={exporting || rendering} onclick={onCurrentPng}>
			PNG <Download size={12} strokeWidth={1.4} />
		</button>
		<div class="flex items-center justify-between px-0.5 py-0.5 font-mono text-[10px] text-muted">
			<span>PNG scale</span>
			<CompactSelectMenu
				label="Choose PNG scale"
				value={`${pngScale}×`}
				items={scaleMenu}
				onAction={onScale}
				align="end"
				class="min-w-12"
			/>
		</div>
	</div>

	<div class="mt-2 flex flex-col gap-1.5 border-b border-subtle pb-2.5">
		<span class="mb-0.5 text-[10px] text-muted">Whole document</span>
		<button class="flex w-full items-center justify-between {secondary}" disabled={exporting} onclick={() => onArchive('svg')}>
			all SVG <span class="opacity-65">.zip</span>
		</button>
		<button class="flex w-full items-center justify-between {secondary}" disabled={exporting} onclick={() => onArchive('png')}>
			all PNG <span class="opacity-65">.zip</span>
		</button>
		<button class="flex w-full items-center justify-between {secondary}" disabled={exporting} onclick={() => onArchive('everything')}>
			everything <span class="opacity-65">.zip</span>
		</button>
	</div>

	<div class="mt-2 flex flex-col gap-1.5">
		<span class="mb-0.5 text-[10px] text-muted">Structure</span>
		<button class="flex w-full items-center justify-between {secondary}" disabled={exporting} onclick={onJson}>
			document JSON <Download size={12} strokeWidth={1.4} />
		</button>
	</div>

	{#if exporting}
		<button class="mt-2 cursor-pointer bg-transparent py-2 font-mono text-[10px] text-muted underline underline-offset-3" type="button" onclick={onCancel}>
			cancel export
		</button>
	{/if}
</aside>
