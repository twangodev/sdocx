<script lang="ts">
	interface Props {
		size?: number;
	}

	const { size = 16 }: Props = $props();
</script>

<svg width={size} height={size} viewBox="0 0 32 32" aria-hidden="true">
	<path
		d="M24 8.5c-2-1.4-4.4-2.2-7.2-2.2-4.8 0-8 2.2-8 5.7 0 3 2.1 4.6 6.7 5.5 3.3.6 4.5 1.3 4.5 2.6 0 1.5-1.6 2.4-4.2 2.4-2.7 0-5.3-.9-7.4-2.4"
		fill="none"
		stroke="currentColor"
		stroke-linecap="round"
		stroke-width="3.5"
	/>
	<rect x="23" y="22" width="3.5" height="3.5" rx="0.7" fill="currentColor" />
</svg>
