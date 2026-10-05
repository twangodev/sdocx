<script lang="ts">
	import type { PageRenderReport } from '$converter/protocol';
	import { diagnosticLabel, renderNoticeCount } from '$converter/render-reports';

	let { reports, label }: { reports: PageRenderReport[]; label: string } = $props();
	let expanded = $state(false);
	const count = $derived(renderNoticeCount(reports));
</script>

{#if count}
	<details bind:open={expanded} class="mt-3 rounded-md border border-subtle p-2.5 text-[11px]" data-render-notices>
		<summary class="cursor-pointer font-medium">{label} · {count} {count === 1 ? 'notice' : 'notices'}</summary>
		{#if expanded}
			<p class="mt-2 text-muted">Some content uses a fallback or could not be drawn as saved.</p>
			<ul class="mt-2 list-none space-y-2 p-0">
				{#each reports as report}
					{#if report.text_diagnostics.length || report.object_diagnostics.length}
						<li>
							<strong>Page {report.page_index + 1}</strong>
							<ul class="mt-1 list-none space-y-1 p-0 text-muted">
								{#each report.text_diagnostics as issue}
									<li>{diagnosticLabel(issue.kind)}{issue.family ? ` · ${issue.family}` : ''}{issue.codepoints.length ? ` · ${issue.codepoints.map(point => `U+${point.toString(16).toUpperCase().padStart(4, '0')}`).join(', ')}` : ''}</li>
								{/each}
								{#each report.object_diagnostics as issue}
									<li>{diagnosticLabel(issue.kind)} · text position {issue.anchor_utf16}</li>
								{/each}
							</ul>
						</li>
					{/if}
				{/each}
			</ul>
		{/if}
	</details>
{/if}
