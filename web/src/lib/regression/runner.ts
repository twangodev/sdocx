import { ConverterClient } from '../converter/client';
import { fetchVerifiedAsset, readVerifiedFileAsset, transferableBuffer } from './fetch';
import { fixtureAssetUrl, type CorpusFixture } from './manifest';
import { renderComparisons, type ComparisonPage } from './pdf';
import { runStructuralChecks, type StructuralCheck } from './structure';

export type FixtureStatus =
	| 'queued'
	| 'downloading'
	| 'parsing'
	| 'checking'
	| 'comparing'
	| 'passed'
	| 'failed'
	| 'cancelled';

export interface FixtureRunResult {
	fixture: CorpusFixture;
	status: FixtureStatus;
	message: string;
	startedAt?: string;
	finishedAt?: string;
	durationMs?: number;
	sdocxSha256?: string;
	referencePdfSha256?: string;
	visiblePageCount?: number;
	referencePageCount?: number;
	checks: StructuralCheck[];
	comparisons: ComparisonPage[];
	error?: string;
}

export interface SuiteExecution {
	startedAt: string;
	finishedAt: string;
	cancelled: boolean;
	results: FixtureRunResult[];
}

export interface LocalFixtureAssets {
	sdocx: File;
	referencePdf: File;
}

export interface RegressionRunOptions {
	localAssets?: ReadonlyMap<string, LocalFixtureAssets>;
}

export type RunListener = (results: FixtureRunResult[]) => void;

function now(): string {
	return new Date().toISOString();
}

function errorMessage(error: unknown): string {
	return error instanceof Error ? error.message : 'The fixture could not be processed.';
}

function isAbort(error: unknown, signal: AbortSignal): boolean {
	return signal.aborted || (error instanceof DOMException && error.name === 'AbortError');
}

function progressMessage(name: string, received: number, total?: number): string {
	const receivedMiB = (received / 1024 / 1024).toFixed(1);
	if (total === undefined || total === 0) return `Downloading ${name}: ${receivedMiB} MiB`;
	return `Downloading ${name}: ${Math.min(100, (received / total) * 100).toFixed(0)}%`;
}

function cloneResults(results: FixtureRunResult[]): FixtureRunResult[] {
	return results.map((result) => ({ ...result }));
}

export class RegressionSuiteRunner {
	private controller?: AbortController;
	private client?: ConverterClient;
	private active?: FixtureRunResult;

	get running(): boolean {
		return this.controller !== undefined;
	}

	cancel(): void {
		this.controller?.abort(new DOMException('The regression run was cancelled.', 'AbortError'));
		this.client?.cancel();
	}

	async run(
		fixtures: CorpusFixture[],
		onUpdate: RunListener,
		options: RegressionRunOptions = {}
	): Promise<SuiteExecution> {
		if (this.running) throw new Error('A regression run is already active.');
		const startedAt = now();
		const results: FixtureRunResult[] = fixtures.map((fixture) => ({
			fixture,
			status: 'queued',
			message: 'Waiting',
			checks: [],
			comparisons: []
		}));
		const notify = () => onUpdate(cloneResults(results));
		this.controller = new AbortController();
		const { signal } = this.controller;
		this.client = new ConverterClient((phase, message) => {
			if (!this.active) return;
			this.active.status = phase === 'rendering' ? 'comparing' : 'parsing';
			this.active.message = message;
			notify();
		});
		notify();

		try {
			for (const result of results) {
				if (signal.aborted) break;
				this.active = result;
				result.startedAt = now();
				const started = performance.now();
				try {
					await this.executeFixture(
						result,
						signal,
						notify,
						options.localAssets?.get(result.fixture.id)
					);
				} catch (error) {
					result.error = errorMessage(error);
					result.status = isAbort(error, signal) ? 'cancelled' : 'failed';
					result.message = result.status === 'cancelled' ? 'Cancelled' : result.error;
				} finally {
					result.finishedAt = now();
					result.durationMs = Math.round(performance.now() - started);
					notify();
				}
			}

			if (signal.aborted) {
				for (const result of results) {
					if (result.status === 'queued') {
						result.status = 'cancelled';
						result.message = 'Not run';
					}
				}
				notify();
			}

			return { startedAt, finishedAt: now(), cancelled: signal.aborted, results };
		} finally {
			this.active = undefined;
			this.client?.destroy();
			this.client = undefined;
			this.controller = undefined;
		}
	}

	private async executeFixture(
		result: FixtureRunResult,
		signal: AbortSignal,
		notify: () => void,
		localAssets?: LocalFixtureAssets
	): Promise<void> {
		const { fixture } = result;
		result.status = 'downloading';
		result.message = localAssets
			? `Reading local ${localAssets.sdocx.name}`
			: `Downloading ${fixture.sdocx}`;
		notify();
		const sdocxProgress = ({ receivedBytes, totalBytes }: { receivedBytes: number; totalBytes?: number }) => {
			result.message = progressMessage(
				localAssets?.sdocx.name ?? fixture.sdocx,
				receivedBytes,
				totalBytes
			);
			notify();
		};
		const sdocx = localAssets
			? await readVerifiedFileAsset(localAssets.sdocx, fixture.sdocxSha256, {
					signal,
					onProgress: sdocxProgress
				})
			: await fetchVerifiedAsset(fixtureAssetUrl(fixture.sdocx), fixture.sdocxSha256, {
					signal,
					onProgress: sdocxProgress
				});
		result.sdocxSha256 = sdocx.sha256;

		result.message = localAssets
			? `Reading local ${localAssets.referencePdf.name}`
			: `Downloading ${fixture.referencePdf}`;
		notify();
		const referenceProgress = ({
			receivedBytes,
			totalBytes
		}: {
			receivedBytes: number;
			totalBytes?: number;
		}) => {
			result.message = progressMessage(
				localAssets?.referencePdf.name ?? fixture.referencePdf,
				receivedBytes,
				totalBytes
			);
			notify();
		};
		const reference = localAssets
			? await readVerifiedFileAsset(localAssets.referencePdf, fixture.referencePdfSha256, {
					signal,
					onProgress: referenceProgress
				})
			: await fetchVerifiedAsset(fixtureAssetUrl(fixture.referencePdf), fixture.referencePdfSha256, {
				signal,
				onProgress: referenceProgress
			});
		result.referencePdfSha256 = reference.sha256;

		result.status = 'parsing';
		result.message = 'Parsing SDOCX in the browser worker';
		notify();
		const summary = await this.requireClient().load(transferableBuffer(sdocx.bytes));
		result.visiblePageCount = summary.pageCount;

		result.status = 'checking';
		result.message = 'Running strict structural checks';
		notify();
		const structural = runStructuralChecks(fixture, summary.inspection, summary.pageCount);
		result.checks = structural.checks;

		result.status = 'comparing';
		result.message = 'Loading PDF.js';
		notify();
		const comparison = await renderComparisons(
			this.requireClient(),
			reference.bytes,
			summary.pageCount,
			signal,
			(completed, total) => {
				result.message = `Comparing page ${completed} of ${total}`;
				notify();
			}
		);
		result.comparisons = comparison.pages;
		result.referencePageCount = comparison.referencePageCount;

		if (!structural.passed) {
			const failures = result.checks.filter((item) => !item.passed).length;
			result.status = 'failed';
			result.error = `${failures} strict structural check${failures === 1 ? '' : 's'} failed.`;
			result.message = result.error;
			return;
		}

		result.status = 'passed';
		result.message = 'Structure passed; visual metrics are informational';
	}

	private requireClient(): ConverterClient {
		if (!this.client) throw new Error('The converter client is unavailable.');
		return this.client;
	}
}
