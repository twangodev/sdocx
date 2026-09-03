<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	interface Props extends HTMLButtonAttributes {
		variant?: 'default' | 'primary' | 'quiet';
		children: Snippet;
	}

	let {
		variant = 'default',
		class: className = '',
		children,
		...rest
	}: Props = $props();

	const variants = {
		default: 'border-subtle bg-transparent text-text hover:border-muted hover:bg-surface',
		primary: 'border-text bg-text text-bg hover:opacity-80',
		quiet: 'border-transparent bg-transparent text-muted hover:bg-surface hover:text-text'
	} as const;
</script>

<button
	{...rest}
	type={rest.type ?? 'button'}
	class="inline-flex min-h-7 cursor-pointer items-center justify-center gap-1.5 rounded border px-2 py-1 text-[0.7rem] transition-[border-color,background-color,color,opacity,transform] duration-[var(--motion-fast)] ease-[var(--ease-standard)] disabled:cursor-not-allowed disabled:opacity-45 {variants[
		variant
	]} {className}"
>
	{@render children()}
</button>
