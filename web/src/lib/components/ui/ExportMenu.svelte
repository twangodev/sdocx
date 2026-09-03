<script lang="ts">
	import { Download, LoaderCircle } from '@lucide/svelte';
	import { separator, type MenuLeaf } from '$lib/menu';
	import DropdownMenu from './DropdownMenu.svelte';
	import IconButton from './IconButton.svelte';

	type Scale = 1 | 2;
	type ArchiveKind = 'svg' | 'png' | 'everything';
	type ExportAction =
		| 'current-svg'
		| 'current-png'
		| 'all-svg'
		| 'all-png'
		| 'everything'
		| 'json'
		| 'scale-1'
		| 'scale-2';

	interface ExportMenuModel {
		exporting: boolean;
		rendering: boolean;
		pngScale: Scale;
	}

	interface ExportMenuActions {
		onScale: (scale: Scale) => void;
		onCurrentSvg: () => void;
		onCurrentPng: () => void;
		onArchive: (kind: ArchiveKind) => void;
		onJson: () => void;
		onCancel: () => void;
	}

	interface Props {
		model: ExportMenuModel;
		actions: ExportMenuActions;
	}

	let { model, actions }: Props = $props();

	const items = $derived(makeItems(model.pngScale, model.rendering));

	function makeItems(scale: Scale, isRendering: boolean): MenuLeaf<ExportAction>[] {
		return [
			{
				kind: 'action',
				label: 'Current page as SVG',
				action: 'current-svg',
				disabled: isRendering
			},
			{
				kind: 'action',
				label: `Current page as PNG (${scale}×)`,
				action: 'current-png',
				disabled: isRendering
			},
			separator(),
			{ kind: 'action', label: 'PNG scale: 1×', action: 'scale-1', checked: scale === 1 },
			{ kind: 'action', label: 'PNG scale: 2×', action: 'scale-2', checked: scale === 2 },
			separator(),
			{ kind: 'action', label: 'All pages as SVG (.zip)', action: 'all-svg' },
			{ kind: 'action', label: 'All pages as PNG (.zip)', action: 'all-png' },
			{ kind: 'action', label: 'SVG, PNG, and JSON (.zip)', action: 'everything' },
			separator(),
			{ kind: 'action', label: 'Document structure (.json)', action: 'json' }
		];
	}

	function run(action: ExportAction): void {
		switch (action) {
			case 'current-svg':
				actions.onCurrentSvg();
				break;
			case 'current-png':
				actions.onCurrentPng();
				break;
			case 'all-svg':
				actions.onArchive('svg');
				break;
			case 'all-png':
				actions.onArchive('png');
				break;
			case 'everything':
				actions.onArchive('everything');
				break;
			case 'json':
				actions.onJson();
				break;
			case 'scale-1':
				actions.onScale(1);
				break;
			case 'scale-2':
				actions.onScale(2);
		}
	}
</script>

{#if model.exporting}
	<IconButton label="Cancel export" tooltip tone="danger" onclick={actions.onCancel}>
		<LoaderCircle class="animate-spin" size={13} strokeWidth={1.4} />
	</IconButton>
{:else}
	<DropdownMenu {items} onAction={run} align="end">
		{#snippet children({ props })}
			<IconButton {...props} label="Export document" title="Export document">
				<Download size={13} strokeWidth={1.4} />
			</IconButton>
		{/snippet}
	</DropdownMenu>
{/if}
