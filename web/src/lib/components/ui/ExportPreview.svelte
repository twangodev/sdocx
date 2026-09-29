<script lang="ts">
	import { LoaderCircle, Maximize, Minus, Plus } from '@lucide/svelte';
	import IconButton from './IconButton.svelte';
	import type { ColorMode } from '$converter/protocol';

	let { pageIndex, colorMode, enabled, busy, pngScale, render }: {
		pageIndex: number;
		colorMode: ColorMode;
		enabled: boolean;
		busy: boolean;
		pngScale?: 1 | 2;
		render: (pageIndex: number, colorMode: ColorMode) => Promise<string>;
	} = $props();
	let url = $state('');
	let error = $state('');
	let loading = $state(false);
	let attempt = $state(0);
	let width = $state(0), height = $state(0);
	let viewportWidth = $state(0), viewportHeight = $state(0);
	let zoom = $state<number | null>(null);
	const fit = $derived(width && height ? Math.max(0.001, Math.min(1, (viewportWidth - 48) / width, (viewportHeight - 48) / height)) : 1);
	const imageScale = $derived(zoom ?? fit);

	$effect(() => {
		const page = pageIndex, mode = colorMode;
		attempt;
		url = ''; error = ''; width = 0; height = 0; zoom = null;
		loading = enabled;
		if (!enabled) return;
		let active = true;
		let ownedUrl = '';
		void render(page, mode).then(svg => {
			if (!active) return;
			ownedUrl = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
			url = ownedUrl;
		}).catch(cause => {
			if (!active) return;
			error = cause instanceof Error ? cause.message : 'Could not load the preview.';
			loading = false;
		});
		return () => { active = false; if (ownedUrl) URL.revokeObjectURL(ownedUrl); };
	});

	function loaded(event: Event) {
		const image = event.currentTarget as HTMLImageElement;
		width = image.naturalWidth; height = image.naturalHeight; loading = false;
	}
	function stepZoom(step: number) {
		zoom = Math.min(2, Math.max(0.1, imageScale + step));
	}
</script>

<div class="preview-stage" bind:clientWidth={viewportWidth} bind:clientHeight={viewportHeight} aria-busy={loading}>
	{#if error}
		<div class="preview-message"><p role="alert">{error}</p><button type="button" class="mt-3 rounded-md border border-subtle px-3 py-1.5 text-text" disabled={busy} onclick={() => attempt++}>Retry preview</button></div>
	{:else if !enabled}
		<p class="preview-message">Choose valid pages to preview.</p>
	{:else}
		{#if loading}<div class="preview-message" role="status"><LoaderCircle size={20} class="mx-auto mb-2 animate-spin" />Preparing page…</div>{/if}
		{#if url}
			<div class="preview-sheet" style:visibility={loading ? 'hidden' : undefined}>
				<img src={url} alt={`Export preview of page ${pageIndex + 1}`} onload={loaded}
					onerror={() => { error = 'Could not display the preview.'; loading = false; }}
					style:width={width ? `${width * imageScale}px` : '100%'}
					style:height={height ? `${height * imageScale}px` : 'auto'} />
			</div>
		{/if}
	{/if}
</div>
<div class="flex min-h-11 flex-wrap items-center justify-between gap-2 border-t border-subtle px-3 py-2 text-[11px] text-muted">
	<span>{#if width && height}{pngScale ? `${width * pngScale} × ${height * pngScale} px output` : `${width} × ${height}`}{:else}Page preview{/if}</span>
	<div class="flex items-center gap-1">
		<IconButton label="Zoom out export preview" disabled={busy || !width || imageScale <= 0.1} onclick={() => stepZoom(-0.25)}><Minus size={13} /></IconButton>
		<button type="button" class="min-w-11 rounded px-1 py-1 hover:bg-surface disabled:opacity-40" disabled={busy || !width} aria-label="Actual size export preview" onclick={() => zoom = 1}>{Math.round(imageScale * 100)}%</button>
		<IconButton label="Zoom in export preview" disabled={busy || !width || imageScale >= 2} onclick={() => stepZoom(0.25)}><Plus size={13} /></IconButton>
		<IconButton label="Fit export preview to page" active={zoom === null} disabled={busy || !width} onclick={() => zoom = null}><Maximize size={13} /></IconButton>
	</div>
</div>

<style>
	.preview-stage { position: relative; flex: 1; min-height: 0; overflow: auto; background: var(--site-canvas); }
	.preview-sheet { display: flex; align-items: center; justify-content: center; min-width: 100%; min-height: 100%; width: max-content; padding: 24px; }
	.preview-sheet img { display: block; max-width: none; flex-shrink: 0; box-shadow: 0 3px 18px #0002; }
	.preview-message { position: absolute; inset: 0; display: flex; flex-direction: column; justify-content: center; align-items: center; padding: 24px; text-align: center; font-size: 12px; color: var(--site-muted); }
</style>
