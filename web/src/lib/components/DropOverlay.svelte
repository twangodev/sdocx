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
	class="drop-overlay"
	class:visible={dragging}
	role="status"
	aria-live="polite"
	aria-hidden={!dragging}
>
	<div class="drop-overlay-copy">
		<strong>{hasDocument ? 'drop to replace document' : 'drop .sdocx to open'}</strong>
		<span>release anywhere · processed locally</span>
	</div>
</div>

<style>
	.drop-overlay {
		position: fixed;
		inset: 0;
		z-index: 100;
		display: grid;
		place-items: center;
		background: color-mix(in srgb, var(--site-bg) 96%, transparent);
		opacity: 0;
		pointer-events: none;
		visibility: hidden;
		transition:
			opacity var(--motion-standard) var(--ease-out),
			visibility 0s linear var(--motion-standard);
	}

	.drop-overlay.visible {
		opacity: 1;
		visibility: visible;
		transition-delay: 0s;
	}

	.drop-overlay::after {
		position: absolute;
		inset: 1rem;
		border: 1px dashed var(--site-muted);
		border-radius: 0.35rem;
		content: '';
		opacity: 0;
		transform: scale(0.992);
		transition:
			opacity var(--motion-standard) var(--ease-out),
			transform var(--motion-panel) var(--ease-out);
	}

	.drop-overlay.visible::after {
		opacity: 1;
		transform: scale(1);
	}

	.drop-overlay-copy {
		display: flex;
		align-items: center;
		flex-direction: column;
		gap: 0.35rem;
		text-align: center;
		opacity: 0;
		transform: translateY(6px) scale(0.985);
		transition:
			opacity var(--motion-standard) var(--ease-out),
			transform var(--motion-panel) var(--ease-out);
	}

	.drop-overlay.visible .drop-overlay-copy {
		opacity: 1;
		transform: translateY(0) scale(1);
	}

	.drop-overlay-copy strong {
		font-size: 1rem;
		font-weight: 550;
		letter-spacing: -0.015em;
	}

	.drop-overlay-copy span {
		color: var(--site-muted);
		font-family: var(--font-mono);
		font-size: 0.65rem;
	}
</style>
