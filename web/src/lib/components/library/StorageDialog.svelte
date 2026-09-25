<script lang="ts">
	import { onMount } from 'svelte';
	import { Dialog } from 'bits-ui';
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
	let busy = $state(false);
	let error = $state('');
	let notice = $state('');
	let confirming = $state(false);
	const originalBytes = $derived(
		library.snapshot.documents.reduce((sum, document) => sum + document.size, 0)
	);
	onMount(() => {
		void refresh();
	});
	async function refresh() {
		try {
			status = await storageStatus();
		} catch (cause) {
			error = errorMessage(cause);
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
			<Dialog.Title class="text-base font-medium">Browser storage</Dialog.Title>
			<Dialog.Description class="mt-2 text-xs leading-relaxed text-muted"
				>Notes are saved on this device, in this browser. Clearing site data removes the library.
				Download originals to keep a separate copy.</Dialog.Description
			>
			<dl class="my-5 space-y-3 text-xs">
				<div class="flex justify-between">
					<dt>Saved notes</dt>
					<dd>
						{library.snapshot.documents.length} · {formatStorageBytes(originalBytes)} originals
					</dd>
				</div>
				<div class="flex justify-between">
					<dt>Estimated site usage</dt>
					<dd>{formatStorageBytes(status?.usage ?? null)}</dd>
				</div>
				<div class="flex justify-between">
					<dt>Browser quota</dt>
					<dd>{formatStorageBytes(status?.quota ?? null)}</dd>
				</div>
				<div class="flex justify-between">
					<dt>Persistence</dt>
					<dd>
						{status?.persisted === true
							? 'Granted'
							: status?.persisted === false
								? 'Best effort'
								: 'Unavailable'}
					</dd>
				</div>
			</dl>
			<p class="text-xs leading-relaxed text-muted">
				Persistent storage reduces automatic eviction; it does not prevent you from clearing site
				data.
			</p>
			<div class="mt-4 flex flex-col gap-2 text-xs">
				<button
					class="storage-action"
					disabled={busy || !status?.canRequestPersistence || status.persisted === true}
					onclick={() =>
						void run(async () => {
							const granted = await requestPersistence();
							notice = granted
								? 'Persistent storage granted.'
								: 'Persistence was not granted. Notes remain saved with best-effort storage.';
						})}>Request persistent storage</button
				>
				<button
					class="storage-action"
					disabled={busy || !library.available || library.importing}
					onclick={() =>
						void run(async () => {
							await library.service.clearThumbnails();
							notice = 'Thumbnails cleared. They will rebuild as you open notes.';
						})}>Clear thumbnail cache</button
				>
				{#if !confirming}<button
						class="storage-action"
						disabled={busy || !library.available || library.importing}
						onclick={() => (confirming = true)}>Delete library…</button
					>{/if}
			</div>
			{#if confirming}
				<div class="mt-4 rounded border border-subtle p-3 text-xs">
					<p>
						Delete all saved notes, collections, and thumbnails from this browser? This cannot be
						undone.
					</p>
					<div class="mt-3 flex gap-2">
						<button class="storage-action" disabled={busy} onclick={() => (confirming = false)}
							>Keep library</button
						><button
							class="storage-action"
							disabled={busy || library.importing}
							onclick={() =>
								void run(async () => {
									await library.service.clearLibrary();
									library.results = [];
									library.selected = [];
									library.selectSource('all');
									confirming = false;
									notice = 'Library deleted.';
								})}>Delete entire library</button
						>
					</div>
				</div>
			{/if}
			{#if error}<p role="alert" class="mt-3 text-xs text-danger">{error}</p>{/if}
			{#if notice}<p role="status" class="mt-3 text-xs">{notice}</p>{/if}
			<div class="mt-5 flex justify-end">
				<button class="storage-action text-xs" disabled={busy} onclick={onClose}>Close</button>
			</div>
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>

<style>
	.storage-action {
		border: 1px solid var(--site-border);
		border-radius: 4px;
		padding: 8px 12px;
		text-align: left;
		cursor: pointer;
	}
	.storage-action:hover {
		background: var(--site-surface);
	}
	.storage-action:disabled {
		opacity: 0.45;
		cursor: default;
	}
</style>
