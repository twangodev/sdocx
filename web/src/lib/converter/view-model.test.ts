import { describe, expect, it } from 'vitest';
import { toInspectionView } from './view-model';

describe('inspection presentation', () => {
	it('reads both document metadata and parser diagnostics', () => {
		const view = toInspectionView({
			document: {
				metadata: {
					format_version: 5500,
					page_dimensions: [1440, 1920],
					media_assets: [{ name: 'image.png' }],
					note_title: { text: 'Field notes' }
				}
			},
			report: {
				diagnostics: [
					{ code: 'UnknownObjectType', message: 'Skipped a future object', archive_entry: '1.page' }
				]
			}
		});

		expect(view.title).toBe('Field notes');
		expect(view.formatVersion).toBe('5500');
		expect(view.dimensions).toBe('1440 × 1920');
		expect(view.mediaCount).toBe(1);
		expect(view.diagnostics).toEqual([
			{
				code: 'UnknownObjectType',
				message: 'Skipped a future object',
				entry: '1.page'
			}
		]);
	});

	it('degrades safely when future inspection fields are absent', () => {
		expect(toInspectionView(null)).toMatchObject({ mediaCount: 0, diagnostics: [] });
	});
});
