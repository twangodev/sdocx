import { CORPUS_FIXTURES } from './manifest';
import {
	CommitRegressionRunner,
	disposeCommitResults,
	emptyCommitResults,
	type CommitFixtureResult,
	type CommitRunSummary
} from './commit-runner';
import type { RendererArtifact, RendererCatalog } from './commit-protocol';
import { loadRendererCatalog, resolveRenderer } from './renderer-catalog';

export class CommitRegressionSession {
	catalog = $state<RendererCatalog>();
	left = $state<RendererArtifact>();
	right = $state<RendererArtifact>();
	results = $state<CommitFixtureResult[]>(emptyCommitResults(CORPUS_FIXTURES));
	summary = $state<CommitRunSummary>();
	preparing = $state(true);
	running = $state(false);
	error = $state('');

	private readonly runner = new CommitRegressionRunner();

	constructor(
		readonly leftRef: string,
		readonly rightRef: string
	) {}

	get ready(): boolean {
		return this.left !== undefined && this.right !== undefined;
	}

	async prepare(): Promise<void> {
		this.preparing = true;
		this.error = '';
		try {
			this.catalog = await loadRendererCatalog();
			this.left = resolveRenderer(this.catalog, this.leftRef);
			this.right = resolveRenderer(this.catalog, this.rightRef);
		} catch (error) {
			this.error = error instanceof Error ? error.message : 'The renderer pair is unavailable.';
		} finally {
			this.preparing = false;
		}
	}

	async run(): Promise<void> {
		if (!this.left || !this.right || this.running) return;
		disposeCommitResults(this.results);
		this.results = emptyCommitResults(CORPUS_FIXTURES);
		this.summary = undefined;
		this.error = '';
		this.running = true;
		try {
			this.summary = await this.runner.run(
				CORPUS_FIXTURES,
				this.left,
				this.right,
				(results) => (this.results = results)
			);
		} catch (error) {
			this.error = error instanceof Error ? error.message : 'The comparison could not start.';
		} finally {
			this.running = false;
		}
	}

	cancel(): void {
		this.runner.cancel();
	}

	destroy(): void {
		this.runner.cancel();
		disposeCommitResults(this.results);
	}
}
