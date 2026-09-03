import { describe, expect, it } from 'vitest';
import { parseRendererCatalog, resolveRenderer } from './renderer-catalog';

const catalog = parseRendererCatalog({
	version: 1,
	renderers: [
		{
			sha: '1234567890abcdef1234567890abcdef12345678',
			refs: ['main', 'HEAD^'],
			directory: '/renderers/1234567890abcdef1234567890abcdef12345678'
		},
		{
			sha: 'abcdef0123456789abcdef0123456789abcdef01',
			refs: ['HEAD'],
			directory: '/renderers/abcdef0123456789abcdef0123456789abcdef01'
		}
	]
});

describe('renderer catalog', () => {
	it('resolves aliases, full hashes, and unique hash prefixes', () => {
		expect(resolveRenderer(catalog, 'main').sha).toBe('1234567890abcdef1234567890abcdef12345678');
		expect(resolveRenderer(catalog, 'abcdef0').refs).toContain('HEAD');
		expect(resolveRenderer(catalog, '1234567890abcdef1234567890abcdef12345678').refs).toContain(
			'main'
		);
	});

	it('rejects unavailable renderers with the preparation command', () => {
		expect(() => resolveRenderer(catalog, 'deadbee')).toThrow('bun run regression:prepare');
	});

	it('rejects unsafe artifact directories', () => {
		expect(() =>
			parseRendererCatalog({
				version: 1,
				renderers: [{ sha: '1234567', refs: ['main'], directory: '/../private' }]
			})
		).toThrow('invalid artifact');
	});
});
