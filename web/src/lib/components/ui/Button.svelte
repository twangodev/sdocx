<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	interface Props extends HTMLButtonAttributes {
		tone?: 'default' | 'primary' | 'ghost' | 'danger';
		size?: 7 | 8 | 9;
		children: Snippet;
	}
	let { tone = 'default', size = 8, children, class: className = '', type = 'button', ...rest }: Props = $props();
	const heights = { 7: 'h-7', 8: 'h-8', 9: 'h-9' } as const;
	const tones = {
		default: 'border-subtle bg-bg text-text hover:bg-surface',
		primary: 'border-transparent bg-text text-bg hover:opacity-85',
		ghost: 'border-transparent text-muted hover:bg-surface hover:text-text',
		danger: 'border-danger/25 bg-danger/10 text-danger hover:bg-danger/20'
	} as const;
</script>

<button {...rest} {type} class="inline-flex shrink-0 cursor-pointer items-center justify-center gap-1.5 rounded border px-2.5 text-xs font-medium transition-colors disabled:cursor-default disabled:opacity-40 {heights[size]} {tones[tone]} {className}">
	{@render children()}
</button>
