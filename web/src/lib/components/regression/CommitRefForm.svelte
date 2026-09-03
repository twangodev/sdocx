<script lang="ts">
	import { ArrowLeftRight, Play } from '@lucide/svelte';
	import IconButton from '$lib/components/ui/IconButton.svelte';

	interface Props {
		leftRef: string;
		rightRef: string;
		disabled?: boolean;
		compact?: boolean;
		onCompare: (leftRef: string, rightRef: string) => void;
	}

	let { leftRef, rightRef, disabled = false, compact = false, onCompare }: Props = $props();
	let left = $state('');
	let right = $state('');

	$effect(() => {
		left = leftRef;
		right = rightRef;
	});

	function submit(event: SubmitEvent): void {
		event.preventDefault();
		const normalizedLeft = left.trim();
		const normalizedRight = right.trim();
		if (normalizedLeft && normalizedRight) onCompare(normalizedLeft, normalizedRight);
	}

	function swap(): void {
		[left, right] = [right, left];
	}
</script>

<form
	class="flex items-end {compact
		? 'gap-1.5 max-[720px]:w-full'
		: 'w-full gap-2.5 max-[560px]:flex-wrap'}"
	onsubmit={submit}
>
	<label class="min-w-0 flex-1">
		<span class="mb-1 text-[10px] tracking-[0.04em] text-muted {compact ? 'sr-only' : 'block'}">from</span>
		<input
			bind:value={left}
			aria-label="From commit"
			spellcheck="false"
			autocapitalize="off"
			placeholder="commit hash"
			class="h-8 w-full rounded border border-subtle bg-surface px-2.5 font-mono text-[11px] text-text outline-none transition-colors placeholder:text-muted/50 hover:border-muted focus:border-control-edge {compact
				? 'max-w-48'
				: ''}"
		/>
	</label>

	<IconButton label="Swap commits" tooltip size={8} onclick={swap} class={compact ? '' : 'mb-0.5'}>
		<ArrowLeftRight size={13} strokeWidth={1.5} />
	</IconButton>

	<label class="min-w-0 flex-1">
		<span class="mb-1 text-[10px] tracking-[0.04em] text-muted {compact ? 'sr-only' : 'block'}">to</span>
		<input
			bind:value={right}
			aria-label="To commit"
			spellcheck="false"
			autocapitalize="off"
			placeholder="commit hash"
			class="h-8 w-full rounded border border-subtle bg-surface px-2.5 font-mono text-[11px] text-text outline-none transition-colors placeholder:text-muted/50 hover:border-muted focus:border-control-edge {compact
				? 'max-w-48'
				: ''}"
		/>
	</label>

	<button
		type="submit"
		{disabled}
		class="flex h-8 shrink-0 cursor-pointer items-center justify-center gap-1.5 rounded bg-text px-3 text-[11px] font-medium text-bg transition-opacity hover:opacity-80 disabled:cursor-not-allowed disabled:opacity-35 {compact
			? ''
			: 'max-[560px]:w-full'}"
	>
		<Play size={11} fill="currentColor" strokeWidth={1.5} />
		compare
	</button>
</form>
