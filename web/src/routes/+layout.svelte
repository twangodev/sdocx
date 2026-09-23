<script lang="ts">
	import { dev } from '$app/environment';
	import { setContext } from 'svelte';
	import { Star, Bug } from '@lucide/svelte';
	import IconButton from '$lib/components/ui/IconButton.svelte';
	import { WORKSPACE, type WorkspaceState } from '$lib/workspace';
	import './layout.css';
	import Logo from '$lib/components/Logo.svelte';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';

	let { children } = $props();
	const workspace = $state<WorkspaceState>({
		hasDocument: false,
		debuggerOpen: false
	});
	setContext(WORKSPACE, workspace);
</script>

<svelte:head>
	{#if !dev}
		<script
			src="https://rybbit.twango.dev/api/script.js"
			data-site-id="84f39267b7e1"
			defer
		></script>
	{/if}
</svelte:head>

<div class="flex min-h-svh w-full flex-col">
	<header
		class="flex min-h-10 items-center justify-between gap-2.5 border-b border-subtle px-2.5"
	>
		<a
			class="inline-flex items-center gap-1.5 text-[0.9rem] font-[550] tracking-[0.01em] text-text no-underline transition-[color,transform] duration-[var(--motion-fast)] ease-[var(--ease-standard)] hover:text-accent"
			href="/"
			aria-label="sdocx home"><Logo size={15} />sdocx</a
		>

		<nav class="flex items-center gap-2" aria-label="Primary navigation">
			{#if workspace.hasDocument}
				<IconButton
					label="Debugger"
					tooltip
					active={workspace.debuggerOpen}
					aria-pressed={workspace.debuggerOpen}
					aria-controls="debugger-sidebar"
					onclick={() => (workspace.debuggerOpen = !workspace.debuggerOpen)}
				>
					<Bug size={15} strokeWidth={1.5} aria-hidden="true" />
				</IconButton>
			{/if}
			<a
				class="inline-flex h-[1.85rem] items-center gap-1.5 rounded border border-subtle px-2 text-[11px] text-muted no-underline transition-[background-color,color,transform] duration-[var(--motion-fast)] ease-[var(--ease-standard)] hover:bg-surface hover:text-text"
				href="https://github.com/twangodev/sdocx"
				target="_blank"
				rel="noreferrer"
				title="Star on GitHub"
				aria-label="Star on GitHub"
				><Star size={13} strokeWidth={1.5} aria-hidden="true" />Star</a
			>
			<ThemeToggle />
		</nav>
	</header>
	<main class="flex min-h-0 flex-1 items-stretch">{@render children()}</main>
</div>
