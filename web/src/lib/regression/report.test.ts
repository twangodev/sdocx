import { strFromU8, unzipSync } from 'fflate';
import { describe, expect, it } from 'vitest';
import type { CorpusFixture } from './manifest';
import {
	createRegressionArchive,
	createRegressionReport,
	regressionReportHtml
} from './report';
import type { SuiteExecution } from './runner';

const fixture: CorpusFixture = {
	id: 'fixture-one',
	sdocx: 'fixture.sdocx',
	sdocxSha256: 'a'.repeat(64),
	referencePdf: 'fixture.pdf',
	referencePdfSha256: 'b'.repeat(64),
	storedPages: 1,
	visiblePages: 1,
	title: '<title>',
	minimumBodyCharacters: 1,
	requiredText: 'body',
	textSections: 1,
	hyperlinks: 0,
	tables: 0,
	codeBlocks: 0,
	requiredLinkTarget: '',
	requiredTableText: '',
	requiredCodeText: ''
};

function execution(): SuiteExecution {
	const image = new Blob([Uint8Array.from([1, 2, 3])], { type: 'image/png' });
	return {
		startedAt: '2026-08-30T00:00:00.000Z',
		finishedAt: '2026-08-30T00:00:01.000Z',
		cancelled: false,
		results: [
			{
				fixture,
				status: 'passed',
				message: 'Passed',
				checks: [
					{ id: 'title', label: '<title>', expected: '<title>', actual: '<title>', passed: true }
				],
				comparisons: [
					{
						pageIndex: 0,
						actualSvgUrl: 'blob:svg',
						actualRasterUrl: 'blob:actual',
						referenceUrl: 'blob:reference',
						heatmapUrl: 'blob:heatmap',
						actualRasterPng: image,
						referencePng: image,
						heatmapPng: image,
						metrics: {
							width: 1,
							height: 1,
							meanAbsoluteError: 0,
							rootMeanSquareError: 0,
							changedPixelRatio: 0,
							changedPixels: 0,
							totalPixels: 1,
							threshold: 16,
							generatedSourceWidth: 1200,
							generatedSourceHeight: 1600,
							referenceSourceWidth: 600,
							referenceSourceHeight: 800,
							generatedAspectRatio: 0.75,
							referenceAspectRatio: 0.75,
							aspectRatioDelta: 0,
							aspectRatioMismatch: false
						}
					}
				]
			}
		]
	};
}

describe('regression reports', () => {
	it('creates a compact report and escapes its standalone HTML', () => {
		const report = createRegressionReport(execution());
		expect(report.passed).toBe(true);
		expect(JSON.stringify(report)).not.toContain('blob:');
		expect(regressionReportHtml(report)).toContain('&lt;title&gt;');
		expect(regressionReportHtml(report)).toContain('1200 × 1600');
	});

	it('packages report files and all three page comparison images', async () => {
		const run = execution();
		const report = createRegressionReport(run);
		const archive = unzipSync(new Uint8Array(await (await createRegressionArchive(report, run)).arrayBuffer()));

		expect(JSON.parse(strFromU8(archive['report.json']))).toMatchObject({ passed: true });
		expect(strFromU8(archive['report.html'])).toContain('SDOCX browser regression report');
		for (const name of ['generated.png', 'reference.png', 'heatmap.png']) {
			expect(Array.from(archive[`fixtures/fixture-one/page-001/${name}`])).toEqual([1, 2, 3]);
		}
	});
});
