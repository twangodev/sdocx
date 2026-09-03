<script lang="ts">
	let { message }: { message: string } = $props();
</script>

<p
	class="motion-surface-in mt-4 border-l-2 border-danger bg-danger/10 px-3.5 py-2.5 font-mono text-[0.72rem] text-text"
	role="alert">{message}</p
>
