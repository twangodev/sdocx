<script lang="ts">
	let { message }: { message: string } = $props();
</script>

<p class="error-notice motion-surface-in" role="alert">{message}</p>

<style>
	.error-notice {
		margin: 1rem 0 0;
		border-left: 2px solid var(--color-danger);
		padding: 0.65rem 0.85rem;
		background: color-mix(in srgb, var(--color-danger) 8%, transparent);
		color: var(--site-text);
		font-family: var(--font-mono);
		font-size: 0.72rem;
	}
</style>
