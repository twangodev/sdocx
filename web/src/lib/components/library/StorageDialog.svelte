<script lang="ts">
	import { onMount } from 'svelte';
	import { Dialog } from 'bits-ui';
	import { X } from '@lucide/svelte';
	import Button from '../ui/Button.svelte';
	import IconButton from '../ui/IconButton.svelte';
	import {
		storageStatus,
		requestPersistence,
		formatStorageBytes,
		type StorageStatus
	} from '$lib/library/browser-storage';
	import { errorMessage } from '$lib/library/model';
	import type { LibraryWorkspace } from '$lib/library/workspace.svelte';
	let { library, onClose }: { library: LibraryWorkspace; onClose: () => void } = $props();
	let status = $state<StorageStatus | null>(null);
	let loading = $state(true);
	let busy = $state(false);
	let error = $state('');
	let notice = $state('');
	let confirming = $state(false);
	const originalBytes = $derived(
		library.snapshot.documents.reduce((sum, document) => sum + document.size, 0)
	);
	const usagePercent = $derived(
		status?.quota && status.usage !== null
			? Math.min(100, (status.usage / status.quota) * 100)
			: null
	);
	onMount(() => {
		void refresh();
	});
	async function refresh() {
		loading = true;
		try {
			status = await storageStatus();
		} catch (cause) {
			error = errorMessage(cause);
		} finally {
			loading = false;
		}
	}
	async function run(action: () => Promise<void>) {
		if (busy) return;
		busy = true;
		error = '';
		notice = '';
		try {
			await action();
		} catch (cause) {
			error = errorMessage(cause);
		} finally {
			await library.refresh().catch(() => {});
			await refresh();
			busy = false;
		}
	}
</script>

<Dialog.Root
	open
	onOpenChange={(open) => {
		if (!open && !busy) onClose();
	}}
>
	<Dialog.Portal>
		<Dialog.Overlay class="fixed inset-0 z-70 bg-black/50" />
		<Dialog.Content
			class="fixed top-1/2 left-1/2 z-80 max-h-[90svh] w-[calc(100%-2rem)] max-w-md -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-md border border-subtle bg-bg p-5 shadow-2xl"
			onEscapeKeydown={(event) => {
				if (busy) event.preventDefault();
			}}
			onInteractOutside={(event) => {
				if (busy) event.preventDefault();
			}}
		>
			<div class="flex items-center justify-between gap-3">
				<Dialog.Title class="text-sm font-medium">Browser storage</Dialog.Title>
				<IconButton label="Close" size={7} disabled={busy} onclick={onClose}
					><X size={14} /></IconButton
				>
			</div>
			<Dialog.Description class="mt-2 text-xs leading-relaxed text-muted"
				>Saved on this device. Clearing site data removes your library.</Dialog.Description
			>
			<div class="mt-5 rounded border border-subtle bg-surface p-3">
				<div class="flex items-baseline justify-between gap-3 text-xs">
					<span class="font-medium"
						>{library.snapshot.documents.length}
						{library.snapshot.documents.length === 1 ? 'note' : 'notes'}</span
					><span class="text-muted">{formatStorageBytes(originalBytes)} originals</span>
				</div>
				{#if usagePercent !== null}<div
						class="mt-3 h-1 overflow-hidden rounded bg-border"
						aria-hidden="true"
					>
						<div class="h-full bg-muted" style:width={`${Math.max(1, usagePercent)}%`}></div>
					</div>{/if}
				<dl class="mt-3 space-y-2 text-[11px] text-muted" aria-busy={loading}>
					<div class="flex justify-between gap-3">
						<dt>Estimated site usage</dt>
						<dd>{loading ? 'Loading…' : formatStorageBytes(status?.usage ?? null)}</dd>
					</div>
					<div class="flex justify-between gap-3">
						<dt>Browser quota</dt>
						<dd>{loading ? 'Loading…' : formatStorageBytes(status?.quota ?? null)}</dd>
					</div>
				</dl>
			</div>
			<div class="py-4">
				<div class="flex justify-between gap-3 text-xs">
					<span class="font-medium">Persistence</span><span class="text-muted"
						>{loading
							? 'Loading…'
							: status?.persisted === true
								? 'Granted'
								: status?.persisted === false
									? 'Best effort'
									: 'Unavailable'}</span
					>
				</div>
				<p class="mt-1 text-[11px] leading-relaxed text-muted">
					Helps protect notes from automatic browser cleanup.
				</p>
				<Button
					class="mt-3"
					size={7}
					disabled={busy || loading || !status?.canRequestPersistence || status.persisted === true}
					onclick={() =>
						void run(async () => {
							const granted = await requestPersistence();
							notice = granted
								? 'Persistent storage granted.'
								: 'Persistence was not granted. Notes remain saved with best-effort storage.';
						})}>Request persistent storage</Button
				>
			</div>
			<div class="flex flex-wrap items-center justify-between gap-3 border-t border-subtle py-4">
				<div>
					<p class="text-xs font-medium">Thumbnails</p>
					<p class="mt-1 text-[11px] text-muted">Rebuild as you open notes.</p>
				</div>
				<Button
					size={7}
					disabled={busy || !library.available || library.importing}
					onclick={() =>
						void run(async () => {
							await library.service.clearThumbnails();
							notice = 'Thumbnails cleared. They will rebuild as you open notes.';
						})}>Clear thumbnail cache</Button
				>
			</div>
			<div class="border-t border-subtle pt-4">
				{#if confirming}
					<p class="text-xs leading-relaxed">
						Delete all saved notes, collections, and thumbnails? This cannot be undone.
					</p>
					<div class="mt-3 flex flex-wrap justify-end gap-2">
						<Button tone="ghost" size={7} disabled={busy} onclick={() => (confirming = false)}
							>Keep library</Button
						>
						<Button
							tone="danger"
							size={7}
							disabled={busy || library.importing}
							onclick={() =>
								void run(async () => {
									await library.service.clearLibrary();
									library.results = [];
									library.selected = [];
									library.selectSource('all');
									confirming = false;
									notice = 'Library deleted.';
								})}>Delete entire library</Button
						>
					</div>
				{:else}
					<div class="flex flex-wrap items-center justify-between gap-3">
						<p class="text-[11px] text-muted">Download originals to keep a separate copy.</p>
						<Button
							tone="danger"
							size={7}
							disabled={busy || !library.available || library.importing}
							onclick={() => (confirming = true)}>Delete library…</Button
						>
					</div>
				{/if}
			</div>
			{#if error}<p role="alert" class="mt-3 text-xs text-danger">{error}</p>{/if}
			{#if notice}<p role="status" class="mt-3 text-xs">{notice}</p>{/if}
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>
