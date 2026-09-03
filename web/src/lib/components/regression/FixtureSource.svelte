<script lang="ts">
	import type { CorpusFixture } from '$lib/regression/manifest';
	import type { LocalFixtureAssets } from '$lib/regression/runner';
	import Button from '$lib/components/ui/Button.svelte';
	import SectionHeading from './SectionHeading.svelte';

	type LocalSelection = Partial<LocalFixtureAssets>;

	interface Props {
		fixture: CorpusFixture;
		selection?: LocalSelection;
		onChoose: (kind: keyof LocalFixtureAssets, file: File) => void;
		onClear: () => void;
	}

	let { fixture, selection, onChoose, onClear }: Props = $props();

	function choose(kind: keyof LocalFixtureAssets, event: Event): void {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (file) onChoose(kind, file);
	}
</script>

<section class="bg-surface/55 px-5 py-5.5" aria-labelledby="local-source-heading">
	<SectionHeading
		id="local-source-heading"
		title="Local fixture fallback"
		description="Use matching files from this device if the dataset cannot be reached."
	>
		{#snippet aside()}
			{#if selection?.sdocx || selection?.referencePdf}
				<Button variant="quiet" class="min-h-0 border-0 p-0 font-mono text-[0.66rem]" onclick={onClear}>
					Clear files
				</Button>
			{/if}
		{/snippet}
	</SectionHeading>

	<div class="grid grid-cols-2 gap-2.5 max-[650px]:grid-cols-1">
		<label
			class="grid min-w-0 cursor-pointer gap-1 rounded-[0.45rem] border border-dashed border-subtle bg-raised p-3.5 transition-[border-color,background-color,transform] duration-[var(--motion-fast)] ease-[var(--ease-standard)] hover:border-accent/65"
		>
			<input
				class="sr-only"
				type="file"
				accept=".sdocx,application/zip"
				onchange={(event) => choose('sdocx', event)}
			/>
			<span class="mono-label text-accent">SDOCX</span>
			<strong class="truncate text-[0.78rem]">{selection?.sdocx?.name ?? fixture.sdocx}</strong>
			<small class="text-[0.68rem] text-muted">
				{selection?.sdocx ? 'Local file selected' : 'Choose local file'}
			</small>
		</label>
		<label
			class="grid min-w-0 cursor-pointer gap-1 rounded-[0.45rem] border border-dashed border-subtle bg-raised p-3.5 transition-[border-color,background-color,transform] duration-[var(--motion-fast)] ease-[var(--ease-standard)] hover:border-accent/65"
		>
			<input
				class="sr-only"
				type="file"
				accept=".pdf,application/pdf"
				onchange={(event) => choose('referencePdf', event)}
			/>
			<span class="mono-label text-accent">Reference PDF</span>
			<strong class="truncate text-[0.78rem]">
				{selection?.referencePdf?.name ?? fixture.referencePdf}
			</strong>
			<small class="text-[0.68rem] text-muted">
				{selection?.referencePdf ? 'Local file selected' : 'Choose local file'}
			</small>
		</label>
	</div>
	<p class="mt-3 mb-0 font-mono text-[0.68rem] leading-5 text-muted">
		Local files must match the manifest SHA-256 values. They never leave this tab.
	</p>
</section>
