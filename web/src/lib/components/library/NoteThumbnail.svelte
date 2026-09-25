<script lang="ts">
	import { FileText } from '@lucide/svelte';
	import type { AssetStore } from '$lib/library/asset-store';
	let { name, assets }: { name: string | null; assets: AssetStore } = $props();
	let url = $state('');
	$effect(() => {
		const thumbnail = name;
		let active = true;
		let objectUrl = '';
		url = '';
		if (thumbnail)
			void assets
				.read(thumbnail)
				.then((file) => {
					if (!active) return;
					objectUrl = URL.createObjectURL(file);
					url = objectUrl;
				})
				.catch(() => {});
		return () => {
			active = false;
			if (objectUrl) URL.revokeObjectURL(objectUrl);
		};
	});
</script>

<div class="thumbnail">
	{#if url}<img src={url} alt="" loading="lazy" />{:else}<FileText size={30} strokeWidth={1} />{/if}
</div>

<style>
	.thumbnail {
		display: grid;
		place-items: center;
		height: 100%;
		min-height: 0;
		background: var(--site-surface);
		color: var(--site-muted);
		overflow: hidden;
	}
	img {
		max-width: 100%;
		max-height: 100%;
		object-fit: contain;
	}
</style>
