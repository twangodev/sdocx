import { createZip, downloadBlob, textBytes } from '../converter/files';
import type { FixtureRunResult, SuiteExecution } from './runner';

export interface RegressionReport {
	format: 1;
	generator: 'sdocx-browser-regressions';
	dataset: 'twangodev/sdocx-compatibility';
	startedAt: string;
	finishedAt: string;
	cancelled: boolean;
	passed: boolean;
	fixtures: Array<{
		id: string;
		status: string;
		durationMs?: number;
		hashes: { sdocx?: string; referencePdf?: string };
		pageCounts: { visible?: number; reference?: number };
		checks: FixtureRunResult['checks'];
		visualMetrics: Array<FixtureRunResult['comparisons'][number]['metrics'] & { page: number }>;
		error?: string;
	}>;
}

export function createRegressionReport(execution: SuiteExecution): RegressionReport {
	return {
		format: 1,
		generator: 'sdocx-browser-regressions',
		dataset: 'twangodev/sdocx-compatibility',
		startedAt: execution.startedAt,
		finishedAt: execution.finishedAt,
		cancelled: execution.cancelled,
		passed: execution.results.length > 0 && execution.results.every((result) => result.status === 'passed'),
		fixtures: execution.results.map((result) => ({
			id: result.fixture.id,
			status: result.status,
			durationMs: result.durationMs,
			hashes: {
				sdocx: result.sdocxSha256,
				referencePdf: result.referencePdfSha256
			},
			pageCounts: {
				visible: result.visiblePageCount,
				reference: result.referencePageCount
			},
			checks: result.checks,
			visualMetrics: result.comparisons.map((page) => ({ page: page.pageIndex + 1, ...page.metrics })),
			error: result.error
		}))
	};
}

function escapeHtml(value: unknown): string {
	return String(value)
		.replaceAll('&', '&amp;')
		.replaceAll('<', '&lt;')
		.replaceAll('>', '&gt;')
		.replaceAll('"', '&quot;')
		.replaceAll("'", '&#039;');
}

function percent(value: number): string {
	return `${(value * 100).toFixed(2)}%`;
}

export function regressionReportHtml(report: RegressionReport): string {
	const fixtures = report.fixtures
		.map((fixture) => {
			const checks = fixture.checks
				.map(
					(item) =>
						`<tr><td>${escapeHtml(item.label)}</td><td>${escapeHtml(item.expected)}</td><td>${escapeHtml(item.actual)}</td><td>${item.passed ? 'Pass' : 'Fail'}</td></tr>`
				)
				.join('');
			const metrics = fixture.visualMetrics
				.map(
					(item) =>
						`<tr><td>${item.page}</td><td>${percent(item.meanAbsoluteError)}</td><td>${percent(item.rootMeanSquareError)}</td><td>${percent(item.changedPixelRatio)}</td><td>${item.generatedSourceWidth} × ${item.generatedSourceHeight}</td><td>${item.referenceSourceWidth.toFixed(1)} × ${item.referenceSourceHeight.toFixed(1)}</td><td>${percent(item.aspectRatioDelta)}${item.aspectRatioMismatch ? ' mismatch' : ''}</td></tr>`
				)
				.join('');
			return `<section><h2>${escapeHtml(fixture.id)} — ${escapeHtml(fixture.status)}</h2>${fixture.error ? `<p>${escapeHtml(fixture.error)}</p>` : ''}<h3>Structural checks</h3><table><thead><tr><th>Check</th><th>Expected</th><th>Actual</th><th>Result</th></tr></thead><tbody>${checks}</tbody></table><h3>Informational visual metrics</h3><table><thead><tr><th>Page</th><th>MAE</th><th>RMSE</th><th>Changed pixels</th><th>Generated source</th><th>Reference source</th><th>Aspect delta</th></tr></thead><tbody>${metrics}</tbody></table></section>`;
		})
		.join('');

	return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>SDOCX regression report</title><style>body{font:14px/1.5 system-ui,sans-serif;max-width:1100px;margin:40px auto;padding:0 24px;color:#172033}h1,h2,h3{line-height:1.2}section{margin:36px 0}table{border-collapse:collapse;width:100%;margin:12px 0 24px}th,td{border:1px solid #ccd3df;padding:8px;text-align:left}th{background:#f4f6fa}</style></head><body><h1>SDOCX browser regression report</h1><p>Dataset: ${escapeHtml(report.dataset)}<br>Started: ${escapeHtml(report.startedAt)}<br>Finished: ${escapeHtml(report.finishedAt)}<br>Result: ${report.passed ? 'Pass' : 'Fail'}</p>${fixtures}</body></html>`;
}

export function downloadReport(report: RegressionReport, format: 'json' | 'html'): void {
	const content =
		format === 'json' ? JSON.stringify(report, null, 2) : regressionReportHtml(report);
	const blob = new Blob([content], {
		type: format === 'json' ? 'application/json' : 'text/html'
	});
	downloadBlob(blob, `sdocx-regression-report.${format}`);
}

async function blobBytes(value: Blob): Promise<Uint8Array<ArrayBuffer>> {
	return new Uint8Array(await value.arrayBuffer());
}

export async function createRegressionArchive(
	report: RegressionReport,
	execution: SuiteExecution
): Promise<Blob> {
	async function* entries(): AsyncGenerator<{ name: string; bytes: Uint8Array }> {
		yield { name: 'report.json', bytes: textBytes(JSON.stringify(report, null, 2)) };
		yield { name: 'report.html', bytes: textBytes(regressionReportHtml(report)) };

		for (const result of execution.results) {
			for (const page of result.comparisons) {
				const directory = `fixtures/${result.fixture.id}/page-${String(page.pageIndex + 1).padStart(3, '0')}`;
				yield {
					name: `${directory}/generated.png`,
					bytes: await blobBytes(page.actualRasterPng)
				};
				yield {
					name: `${directory}/reference.png`,
					bytes: await blobBytes(page.referencePng)
				};
				yield {
					name: `${directory}/heatmap.png`,
					bytes: await blobBytes(page.heatmapPng)
				};
			}
		}
	}

	return createZip(entries());
}

export async function downloadRegressionArchive(
	report: RegressionReport,
	execution: SuiteExecution
): Promise<void> {
	const archive = await createRegressionArchive(report, execution);
	downloadBlob(archive, 'sdocx-regression-report.zip');
}
