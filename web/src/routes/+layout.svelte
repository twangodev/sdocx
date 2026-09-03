<script lang="ts">
	import './layout.css';
	import Logo from '$lib/components/Logo.svelte';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';

	let { children } = $props();
</script>

<div class="flex min-h-svh w-full flex-col">
	<header class="flex min-h-10 items-center justify-between gap-2.5 border-b border-subtle px-2.5">
		<a
			class="inline-flex items-center gap-1.5 text-[0.9rem] font-[550] tracking-[0.01em] text-text no-underline transition-[color,transform] duration-[var(--motion-fast)] ease-[var(--ease-standard)] hover:text-accent"
			href="/"
			aria-label="sdocx home"><Logo size={15} />sdocx</a
		>
		<nav class="flex items-center gap-2" aria-label="Primary navigation">
			<a
				class="font-mono text-[0.68rem] text-muted no-underline lowercase transition-colors duration-[var(--motion-fast)] ease-[var(--ease-standard)] hover:text-accent"
				href="https://github.com/twangodev/sdocx"
				target="_blank"
				rel="noreferrer">github</a
			>
			<ThemeToggle />
		</nav>
	</header>
	<main class="flex min-h-0 flex-1 items-stretch">{@render children()}</main>
</div>
