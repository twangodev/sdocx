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

<section class="intro motion-surface-in" aria-labelledby="intro-title">
	<h1 id="intro-title">Open a Samsung Notes file</h1>
	<p class="lede">Preview and export .sdocx documents. Files stay in this browser.</p>

	<button type="button" class="drop-zone" onclick={onOpen}>
		{#if !parsing}<FolderOpen size={13} strokeWidth={1.4} />{/if}
		{parsing ? status : 'open .sdocx'}
	</button>
	<p class="drop-hint">{parsing ? 'processing locally' : 'or drop one here · max 250 MiB'}</p>

	{#if parsing}
		<button class="cancel-link" type="button" onclick={onCancel}>cancel parsing</button>
	{/if}
	{#if error}<ErrorNotice message={error} />{/if}
</section>

<style>
	.intro {
		width: min(100% - 2rem, 25rem);
		margin: auto;
	}

	h1 {
		margin: 0;
		font-size: 1.3rem;
		font-weight: 550;
		letter-spacing: -0.025em;
		line-height: 1.15;
	}

	.lede {
		margin: 0.45rem 0 1.15rem;
		color: var(--site-muted);
		font-size: 0.875rem;
		line-height: 1.5;
	}

	.drop-zone {
		display: inline-flex;
		width: 100%;
		height: 2.15rem;
		align-items: center;
		justify-content: center;
		gap: 0.45rem;
		border: 0;
		border-radius: 0.25rem;
		background: var(--site-text);
		padding: 0 1rem;
		color: var(--site-bg);
		font-size: 0.75rem;
		font-weight: 550;
		text-align: center;
		cursor: pointer;
		transition:
			opacity var(--motion-fast) var(--ease-standard),
			transform var(--motion-control) var(--ease-out);
	}

	.drop-zone:hover,
	.drop-zone:focus-visible {
		opacity: 0.82;
	}

	.drop-hint {
		margin: 0.5rem 0 0;
		color: var(--site-muted);
		font-size: 0.7rem;
		text-align: center;
	}

	.cancel-link {
		border: 0;
		background: transparent;
		padding: 0.75rem 0;
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.68rem;
		text-decoration: underline;
		text-underline-offset: 3px;
		cursor: pointer;
	}
</style>
