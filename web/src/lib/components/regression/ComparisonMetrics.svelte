<script lang="ts">
	import type { PageVisualMetrics } from '$lib/regression/metrics';

	let { metrics }: { metrics: PageVisualMetrics } = $props();
</script>

<div class="mt-3 grid grid-cols-4 gap-2 max-[650px]:grid-cols-1">
	<div class="rounded border border-subtle bg-surface p-2.5">
		<span class="block font-mono text-[0.58rem] text-muted">Mean absolute error</span>
		<strong class="mt-1 block text-[0.82rem] tabular-nums">
			{(metrics.meanAbsoluteError * 100).toFixed(2)}%
		</strong>
	</div>
	<div class="rounded border border-subtle bg-surface p-2.5">
		<span class="block font-mono text-[0.58rem] text-muted">Root mean square</span>
		<strong class="mt-1 block text-[0.82rem] tabular-nums">
			{(metrics.rootMeanSquareError * 100).toFixed(2)}%
		</strong>
	</div>
	<div class="rounded border border-subtle bg-surface p-2.5">
		<span class="block font-mono text-[0.58rem] text-muted">Changed pixels</span>
		<strong class="mt-1 block text-[0.82rem] tabular-nums">
			{(metrics.changedPixelRatio * 100).toFixed(2)}%
		</strong>
	</div>
	<div class="rounded border border-subtle bg-surface p-2.5">
		<span class="block font-mono text-[0.58rem] text-muted">Raster size</span>
		<strong class="mt-1 block text-[0.82rem] tabular-nums">
			{metrics.width} × {metrics.height}
		</strong>
	</div>
	<div class="rounded border border-subtle bg-surface p-2.5">
		<span class="block font-mono text-[0.58rem] text-muted">Generated source</span>
		<strong class="mt-1 block text-[0.82rem] tabular-nums">
			{metrics.generatedSourceWidth} × {metrics.generatedSourceHeight}
		</strong>
	</div>
	<div class="rounded border border-subtle bg-surface p-2.5">
		<span class="block font-mono text-[0.58rem] text-muted">Reference source</span>
		<strong class="mt-1 block text-[0.82rem] tabular-nums">
			{metrics.referenceSourceWidth.toFixed(1)} × {metrics.referenceSourceHeight.toFixed(1)}
		</strong>
	</div>
	<div class="rounded border border-subtle bg-surface p-2.5">
		<span class="block font-mono text-[0.58rem] text-muted">Aspect ratio delta</span>
		<strong
			class="mt-1 block text-[0.82rem] tabular-nums {metrics.aspectRatioMismatch
				? 'text-negative'
				: ''}"
		>
			{(metrics.aspectRatioDelta * 100).toFixed(2)}%
			{metrics.aspectRatioMismatch ? 'mismatch' : 'match'}
		</strong>
	</div>
</div>
<p class="mt-3 mb-0 text-[0.68rem] leading-5 text-muted">
	Visual metrics are informational. Source sizes use SVG units and PDF points; their aspect ratios
	are comparable. Fonts, rasterization, and antialiasing can change pixels without a parser
	regression.
</p>
