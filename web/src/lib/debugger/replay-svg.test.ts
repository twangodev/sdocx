import { expect, it } from 'vitest';
import { ReplaySvg } from './replay-svg';
import type { Replay, ReplayStroke } from './model';

function strokeNode(index: number) {
	const parts = [1, 2, 3].map(part => ({ dataset: { replayPart: String(part) }, style: { display: '' } }));
	const attributes: Record<string, string> = { d: 'M0,0h1M1,1h2M2,2h3' };
	const path = {
		tagName: 'path', dataset: { replayLengths: '6,12,18' },
		getAttribute: (name: string) => attributes[name],
		setAttribute: (name: string, value: string) => { attributes[name] = value; }
	};
	return {
		dataset: { replayStroke: String(index) }, style: { display: '' }, parts, attributes,
		querySelectorAll: (selector: string) => selector === '[data-replay-part]' ? parts : [path]
	};
}

it('reveals saved sample boundaries and restores full vectors after backward seeks', () => {
	const nodes = [strokeNode(0), strokeNode(1)];
	const root = { querySelectorAll: () => nodes } as unknown as SVGSVGElement;
	const item = {
		stroke: { points: [{}, {}, {}, {}] },
		geometry: { sample_ends: [0, 2, 2, 3] }
	} as ReplayStroke;
	const replay = new ReplaySvg(root, { strokes: [item, item] } as Replay);
	for (const [sample, count] of [[0, 0], [1, 2], [2, 2], [3, 3], [1, 2]]) {
		replay.seek(-1, sample);
		expect(nodes[0].parts.filter(p => p.style.display !== 'none')).toHaveLength(count);
		expect(nodes[0].attributes.d).toBe('M0,0h1M1,1h2M2,2h3'.slice(0, count * 6));
		expect(nodes[1].style.display).toBe('none');
	}
	replay.seek(0, 0);
	expect(nodes[0].attributes.d).toBe('M0,0h1M1,1h2M2,2h3');
	expect(nodes[0].parts.every(p => p.style.display === '')).toBe(true);
	expect(nodes[1].attributes.d).toBe('');
	replay.seek(-1, 1);
	expect(nodes[1].attributes.d).toBe('M0,0h1M1,1h2M2,2h3');
	replay.seek(1, -1);
	expect(nodes.every(n => n.style.display === '' && n.attributes.d.length === 18)).toBe(true);
});

it('rejects mismatched SVG stroke identities instead of revealing the wrong ink', () => {
	const root = { querySelectorAll: () => [strokeNode(1)] } as unknown as SVGSVGElement;
	expect(() => new ReplaySvg(root, { strokes: [{}] } as Replay)).toThrow('does not match');
});
