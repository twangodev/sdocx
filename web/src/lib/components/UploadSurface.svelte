<script lang="ts">
	import { FolderOpen } from '@lucide/svelte';
	import ErrorNotice from './ErrorNotice.svelte';

	interface Props {
		parsing: boolean;
		status: string;
		error: string;
		onOpen: () => void;
		onCancel: () => void;
	}

	let { parsing, status, error, onOpen, onCancel }: Props = $props();
</script>

<section class="intro motion-surface-in m-auto w-[min(calc(100%_-_2rem),25rem)]" aria-labelledby="intro-title">
	<h1 class="m-0 text-[1.3rem] leading-[1.15] font-[550] tracking-[-0.025em]" id="intro-title">
		Open a Samsung Notes file
	</h1>
	<p class="lede mt-2 mb-4.5 text-sm leading-6 text-muted">
		Preview and export .sdocx documents. Files stay in this browser.
	</p>

	<button
		type="button"
		class="inline-flex h-8.5 w-full cursor-pointer items-center justify-center gap-2 rounded border-0 bg-text px-4 text-center text-xs font-[550] text-bg transition-[opacity,transform] duration-[var(--motion-fast)] ease-[var(--ease-standard)] hover:opacity-80 focus-visible:opacity-80"
		onclick={onOpen}
	>
		{#if !parsing}<FolderOpen size={13} strokeWidth={1.4} />{/if}
		{parsing ? status : 'open .sdocx'}
	</button>
	<p class="mt-2 text-center text-[0.7rem] text-muted">
		{parsing ? 'processing locally' : 'or drop one here · max 250 MiB'}
	</p>

	{#if parsing}
		<button
			class="cursor-pointer border-0 bg-transparent py-3 font-mono text-[0.68rem] text-muted underline underline-offset-3"
			type="button"
			onclick={onCancel}>cancel parsing</button
		>
	{/if}
	{#if error}<ErrorNotice message={error} />{/if}
</section>
