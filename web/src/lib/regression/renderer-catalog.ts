import type { RendererArtifact, RendererCatalog } from './commit-protocol';

export const CURRENT_RENDERER: RendererArtifact = {
	sha: 'current',
	refs: ['current'],
	directory: '/wasm'
};

function isRecord(value: unknown): value is Record<string, unknown> {
	return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function cleanArtifact(value: unknown): RendererArtifact | undefined {
	if (!isRecord(value)) return undefined;
	const { sha, refs, directory } = value;
	if (
		typeof sha !== 'string' ||
		!/^[a-f0-9]{7,40}$/i.test(sha) ||
		!Array.isArray(refs) ||
		!refs.every((ref) => typeof ref === 'string' && ref.length > 0 && ref.length <= 80) ||
		typeof directory !== 'string' ||
		!directory.startsWith('/') ||
		directory.includes('..')
	) {
		return undefined;
	}
	return { sha: sha.toLowerCase(), refs, directory: directory.replace(/\/$/, '') };
}

export function parseRendererCatalog(value: unknown): RendererCatalog {
	if (!isRecord(value) || value.version !== 1 || !Array.isArray(value.renderers)) {
		throw new Error('The renderer manifest is invalid.');
	}
	const renderers = value.renderers.flatMap((entry) => {
		const artifact = cleanArtifact(entry);
		return artifact ? [artifact] : [];
	});
	if (renderers.length !== value.renderers.length) {
		throw new Error('The renderer manifest contains an invalid artifact.');
	}
	return {
		version: 1,
		generatedAt: typeof value.generatedAt === 'string' ? value.generatedAt : undefined,
		renderers
	};
}

export async function loadRendererCatalog(fetcher: typeof fetch = fetch): Promise<RendererCatalog> {
	try {
		const response = await fetcher('/renderers/manifest.json', { cache: 'no-cache' });
		if (!response.ok) return { version: 1, renderers: [CURRENT_RENDERER] };
		const catalog = parseRendererCatalog(await response.json());
		return {
			...catalog,
			renderers: [CURRENT_RENDERER, ...catalog.renderers.filter((item) => item.sha !== 'current')]
		};
	} catch {
		return { version: 1, renderers: [CURRENT_RENDERER] };
	}
}

export function resolveRenderer(catalog: RendererCatalog, ref: string): RendererArtifact {
	const target = ref.trim().toLowerCase();
	const exact = catalog.renderers.filter(
		(artifact) =>
			artifact.sha.toLowerCase() === target ||
			artifact.refs.some((alias) => alias.toLowerCase() === target)
	);
	if (exact.length === 1) return exact[0];

	if (/^[a-f0-9]{7,40}$/.test(target)) {
		const prefixed = catalog.renderers.filter((artifact) => artifact.sha.startsWith(target));
		if (prefixed.length === 1) return prefixed[0];
		if (prefixed.length > 1) throw new Error(`Commit ${ref} is ambiguous in this build.`);
	}

	throw new Error(
		`Renderer ${ref} is not in this build. Prepare it locally with bun run regression:prepare -- ${ref} <other-ref>.`
	);
}

export function rendererUrls(artifact: RendererArtifact, origin: string): {
	moduleUrl: string;
	wasmUrl: string;
} {
	const base = new URL(`${artifact.directory.replace(/\/$/, '')}/`, origin);
	return {
		moduleUrl: new URL('sdocx_wasm.js', base).href,
		wasmUrl: new URL('sdocx_wasm_bg.wasm', base).href
	};
}
