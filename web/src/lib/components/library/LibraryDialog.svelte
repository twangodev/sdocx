<script lang="ts">
	import { Dialog } from 'bits-ui';
	import Button from '../ui/Button.svelte';
	import type { Snippet } from 'svelte';
	import { errorMessage } from '$lib/library/model';
	let {
		title,
		description,
		confirmLabel,
		destructive = false,
		onConfirm,
		onClose,
		children
	}: {
		title: string;
		description: string;
		confirmLabel: string;
		destructive?: boolean;
		onConfirm: () => Promise<void>;
		onClose: () => void;
		children?: Snippet;
	} = $props();
	let busy = $state(false);
	let error = $state('');
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		busy = true;
		error = '';
		try {
			await onConfirm();
			onClose();
		} catch (cause) {
			error = errorMessage(cause);
		} finally {
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
			class="fixed top-1/2 left-1/2 z-80 w-[calc(100%-2rem)] max-w-md -translate-x-1/2 -translate-y-1/2 rounded-md border border-subtle bg-bg p-5 shadow-2xl"
			onEscapeKeydown={(event) => {
				if (busy) event.preventDefault();
			}}
			onInteractOutside={(event) => {
				if (busy) event.preventDefault();
			}}
		>
			<Dialog.Title class="text-base font-medium">{title}</Dialog.Title>
			<Dialog.Description class="mt-2 text-xs leading-relaxed text-muted"
				>{description}</Dialog.Description
			>
			<form onsubmit={submit}>
				{#if children}<div class="mt-4">{@render children()}</div>{/if}
				{#if error}<p role="alert" class="mt-3 text-xs text-danger">{error}</p>{/if}
				<div class="mt-5 flex justify-end gap-2 text-xs">
					<Button tone="ghost" disabled={busy} onclick={onClose}>Cancel</Button>
					<Button type="submit" tone={destructive ? 'danger' : 'primary'} disabled={busy}>{busy ? 'Working…' : confirmLabel}</Button>
				</div>
			</form>
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>
