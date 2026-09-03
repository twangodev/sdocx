import { fetchVerifiedAsset } from './fetch';
import { fixtureAssetUrl, type CorpusFixture } from './manifest';
import type { RendererArtifact } from './commit-protocol';
import {
	attachPageUrls,
	compareRenderedDocuments,
	disposeDocumentDiff,
	type DocumentDiff
} from './commit-compare';
import { VersionedRenderer } from './versioned-renderer';

export type CommitFixtureStatus =
	| 'idle'
	| 'queued'
	| 'downloading'
	| 'rendering'
	| 'unchanged'
	| 'changed'
	| 'failed'
	| 'cancelled';

export interface CommitFixtureResult {
	fixture: CorpusFixture;
	status: CommitFixtureStatus;
	message: string;
	durationMs?: number;
	diff?: DocumentDiff;
	error?: string;
}

export interface CommitRunSummary {
	durationMs: number;
	changed: number;
	unchanged: number;
	failed: number;
}

export type CommitRunListener = (results: CommitFixtureResult[]) => void;

function errorMessage(error: unknown): string {
	return error instanceof Error ? error.message : 'The comparison failed.';
}

function initialResults(fixtures: readonly CorpusFixture[]): CommitFixtureResult[] {
	return fixtures.map((fixture) => ({ fixture, status: 'queued', message: 'waiting' }));
}

export function emptyCommitResults(fixtures: readonly CorpusFixture[]): CommitFixtureResult[] {
	return fixtures.map((fixture) => ({ fixture, status: 'idle', message: 'not run' }));
}

export function disposeCommitResults(results: readonly CommitFixtureResult[]): void {
	for (const result of results) disposeDocumentDiff(result.diff);
}

export class CommitRegressionRunner {
	private controller?: AbortController;
	private leftRenderer?: VersionedRenderer;
	private rightRenderer?: VersionedRenderer;

	get running(): boolean {
		return this.controller !== undefined;
	}

	cancel(): void {
		this.controller?.abort(new DOMException('Comparison cancelled.', 'AbortError'));
		this.destroyRenderers();
	}

	async run(
		fixtures: readonly CorpusFixture[],
		left: RendererArtifact,
		right: RendererArtifact,
		onUpdate: CommitRunListener
	): Promise<CommitRunSummary> {
		if (this.running) throw new Error('A comparison is already running.');
		const started = performance.now();
		const results = initialResults(fixtures);
		const notify = () => onUpdate([...results]);
		this.controller = new AbortController();
		const { signal } = this.controller;
		this.leftRenderer = new VersionedRenderer();
		this.rightRenderer = new VersionedRenderer();
		notify();

		try {
			await Promise.all([
				this.leftRenderer.initialize(left),
				this.rightRenderer.initialize(right)
			]);

			for (const result of results) {
				if (signal.aborted) break;
				const fixtureStarted = performance.now();
				try {
					result.status = 'downloading';
					result.message = `downloading ${result.fixture.sdocx}`;
					notify();
					const asset = await fetchVerifiedAsset(
						fixtureAssetUrl(result.fixture.sdocx),
						result.fixture.sdocxSha256,
						{ signal }
					);
					result.status = 'rendering';
					result.message = 'rendering both commits';
					notify();
					const [leftDocument, rightDocument] = await Promise.all([
						this.leftRenderer.render(asset.bytes),
						this.rightRenderer.render(asset.bytes)
					]);
					result.diff = attachPageUrls(
						compareRenderedDocuments(result.fixture, leftDocument, rightDocument)
					);
					result.status = result.diff.changed ? 'changed' : 'unchanged';
					result.message = result.diff.changed
						? `${result.diff.changedPageCount} changed page${result.diff.changedPageCount === 1 ? '' : 's'}`
						: 'no changes';
				} catch (error) {
					result.status = signal.aborted ? 'cancelled' : 'failed';
					result.error = errorMessage(error);
					result.message = signal.aborted ? 'cancelled' : result.error;
				} finally {
					result.durationMs = Math.round(performance.now() - fixtureStarted);
					notify();
				}
			}

			if (signal.aborted) {
				for (const result of results) {
					if (result.status === 'queued') {
						result.status = 'cancelled';
						result.message = 'not run';
					}
				}
				notify();
			}

			return {
				durationMs: Math.round(performance.now() - started),
				changed: results.filter((result) => result.status === 'changed').length,
				unchanged: results.filter((result) => result.status === 'unchanged').length,
				failed: results.filter((result) => result.status === 'failed').length
			};
		} finally {
			this.destroyRenderers();
			this.controller = undefined;
		}
	}

	private destroyRenderers(): void {
		this.leftRenderer?.destroy();
		this.rightRenderer?.destroy();
		this.leftRenderer = undefined;
		this.rightRenderer = undefined;
	}
}
