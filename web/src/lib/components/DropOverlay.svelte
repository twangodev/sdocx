<script lang="ts">
	let { hasDocument, onFile }: { hasDocument: boolean; onFile: (file: File) => void } = $props();
	let dragging = $state(false);
	let dragDepth = 0;

	function carriesFiles(event: DragEvent): boolean {
		return Array.from(event.dataTransfer?.types ?? []).includes('Files');
	}

	function handleDragEnter(event: DragEvent): void {
		if (!carriesFiles(event)) return;
		event.preventDefault();
		dragDepth += 1;
		dragging = true;
	}

	function handleDragOver(event: DragEvent): void {
		if (!carriesFiles(event)) return;
		event.preventDefault();
		if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy';
		dragging = true;
	}

	function handleDragLeave(event: DragEvent): void {
		if (!dragging) return;
		event.preventDefault();
		dragDepth = Math.max(0, dragDepth - 1);
		if (dragDepth === 0) dragging = false;
	}

	function handleDrop(event: DragEvent): void {
		const fileDrop = carriesFiles(event);
		if (fileDrop) event.preventDefault();
		dragDepth = 0;
		dragging = false;

		const file = event.dataTransfer?.files[0];
		if (fileDrop && file) onFile(file);
	}
</script>

<svelte:window
	ondragenter={handleDragEnter}
	ondragover={handleDragOver}
	ondragleave={handleDragLeave}
	ondrop={handleDrop}
/>

<div
	class="drop-overlay pointer-events-none fixed inset-0 z-100 grid place-items-center [background:color-mix(in_srgb,var(--site-bg)_96%,transparent)] {dragging
		? 'visible opacity-100 [transition:opacity_var(--motion-standard)_var(--ease-out),visibility_0s_linear_0s]'
		: 'invisible opacity-0 [transition:opacity_var(--motion-standard)_var(--ease-out),visibility_0s_linear_var(--motion-standard)]'}"
	role="status"
	aria-live="polite"
	aria-hidden={!dragging}
>
	<div
		class="pointer-events-none absolute inset-4 rounded-[0.35rem] border border-dashed border-muted transition-[opacity,transform] duration-[var(--motion-panel)] ease-[var(--ease-out)] {dragging
			? 'scale-100 opacity-100'
			: 'scale-[0.992] opacity-0'}"
	></div>
	<div
		class="flex flex-col items-center gap-1.5 text-center transition-[opacity,transform] duration-[var(--motion-panel)] ease-[var(--ease-out)] {dragging
			? 'translate-y-0 scale-100 opacity-100'
			: 'translate-y-1.5 scale-[0.985] opacity-0'}"
	>
		<strong class="text-base font-[550] tracking-[-0.015em]">
			{hasDocument ? 'drop to replace document' : 'drop .sdocx to open'}
		</strong>
		<span class="font-mono text-[0.65rem] text-muted">release anywhere · processed locally</span>
	</div>
</div>
