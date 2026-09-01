<script lang="ts">
	interface Props {
		pageIndex: number;
		pageCount: number;
		disabled?: boolean;
		onSelect: (page: number) => void;
	}

	let { pageIndex, pageCount, disabled = false, onSelect }: Props = $props();
	let editing = $state(false);
	let draft = $state('');
	const value = $derived(editing ? draft : String(pageIndex + 1));

	function update(event: Event): void {
		draft = (event.currentTarget as HTMLInputElement).value.replace(/\D/g, '');
	}

	function commit(): void {
		const requested = Number.parseInt(draft, 10);
		const nextPage = Number.isFinite(requested)
			? Math.min(pageCount, Math.max(1, requested))
			: pageIndex + 1;
		draft = String(nextPage);
		if (nextPage - 1 !== pageIndex) onSelect(nextPage - 1);
	}

	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter') {
			event.preventDefault();
			commit();
			(event.currentTarget as HTMLInputElement).select();
		} else if (event.key === 'Escape') {
			draft = String(pageIndex + 1);
			(event.currentTarget as HTMLInputElement).blur();
		}
	}
</script>

<div class="flex h-6 items-center gap-1 rounded px-1 font-mono text-[11px] tabular-nums text-text">
	<span class="text-muted">page</span>
	<input
		type="text"
		inputmode="numeric"
		pattern="[0-9]*"
		aria-label="Page number"
		{value}
		{disabled}
		class="h-5 w-6 rounded border border-subtle bg-transparent px-0.5 text-center text-text outline-none transition-[border-color,background-color] duration-150 ease-out focus:border-control-edge focus:bg-raised disabled:opacity-40"
		onfocus={(event) => {
			draft = String(pageIndex + 1);
			editing = true;
			event.currentTarget.select();
		}}
		oninput={update}
		onkeydown={handleKeydown}
		onblur={() => {
			commit();
			editing = false;
		}}
	/>
	<span class="text-muted">/ {pageCount}</span>
</div>
