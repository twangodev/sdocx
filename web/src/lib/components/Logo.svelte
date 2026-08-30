<script lang="ts">
	interface Props {
		size?: number;
	}

	const { size = 16 }: Props = $props();
</script>

<svg width={size} height={size} viewBox="0 0 32 32" fill="currentColor" aria-hidden="true">
	<path
		fill-rule="evenodd"
		d="M6 2h13l7 7v21H6V2Zm2 2v24h16V11h-7V4H8Zm11 1.5V9h3.5L19 5.5ZM11 15h10v2H11v-2Zm0 5h10v2H11v-2Zm0 5h7v2h-7v-2Z"
		clip-rule="evenodd"
	/>
</svg>
