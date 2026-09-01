<script lang="ts">
	import { RefreshCw, X } from '@lucide/svelte';
	import IconButton from './ui/IconButton.svelte';

	interface Props {
		title: string;
		filename: string;
		fileSize: number;
		pageCount: number;
		onReplace: () => void;
		onClose: () => void;
	}

	let { title, filename, fileSize, pageCount, onReplace, onClose }: Props = $props();

	function formatBytes(bytes: number): string {
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
		return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
	}
</script>

<div class="flex min-h-[2.85rem] items-center justify-between gap-2 border-b border-subtle px-3 py-1.5">
	<div class="min-w-0">
		<h1 class="max-w-[60vw] truncate text-[14px] font-[550] leading-tight">{title}</h1>
		<p class="mt-0.5 truncate font-mono text-[10px] text-muted">
			{filename} · {formatBytes(fileSize)} · {pageCount} {pageCount === 1 ? 'page' : 'pages'} · local
		</p>
	</div>
	<div class="flex shrink-0 gap-1">
		<IconButton label="Replace document" tooltip onclick={onReplace}>
			<RefreshCw size={12} strokeWidth={1.4} />
		</IconButton>
		<IconButton label="Close document" tooltip onclick={onClose}>
			<X size={13} strokeWidth={1.4} />
		</IconButton>
	</div>
</div>
