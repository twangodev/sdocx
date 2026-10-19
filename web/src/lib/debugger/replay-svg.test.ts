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

it('reveals captured InkPen V1 source-event prefixes with terminal suffix at the last sample', () => {
	const polygon = (center: number) => {
		const vertices = Array.from({ length: 96 }, (_, index) => {
			const angle = 2 * Math.PI * index / 96;
			return `${(center + Math.cos(angle)).toFixed(7)},${Math.sin(angle).toFixed(7)}`;
		});
		return `M${vertices[0]} ${vertices.slice(1).map(vertex => `L${vertex}`).join(' ')} Z`;
	};
	const stamps = Array.from({ length: 25 }, (_, index) => polygon(index * 2));
	const data = stamps.join(' ');
	const parts: { dataset: { replayPart: string }; style: { display: string } }[] = [];
	const attributes: Record<string, string> = { d: data };
	const path = {
		tagName: 'path', dataset: { replayLengths: stamps.map((_, index) => stamps.slice(0, index + 1).join(' ').length).join(',') },
		getAttribute: (name: string) => attributes[name],
		setAttribute: (name: string, value: string) => { attributes[name] = value; }
	};
	const stroke = {
		dataset: { replayStroke: '0' }, style: { display: '' },
		querySelectorAll: (selector: string) => selector === '[data-replay-part]' ? parts : [path]
	};
	const root = { querySelectorAll: () => [stroke] } as unknown as SVGSVGElement;
	const item = {
		stroke: { points: [{}, {}, {}, {}] },
		// Native saved redraw ends are [0,0,0,16] and terminal is [16,25];
		// page replay folds that suffix into the final source event.
		geometry: { sample_ends: [0, 0, 0, 25] }
	} as ReplayStroke;
	const replay = new ReplaySvg(root, { strokes: [item] } as Replay);
	for (const [sample, expected] of [[0, ''], [1, ''], [2, ''], [3, data]]) {
		replay.seek(-1, sample);
		expect(attributes.d).toBe(expected);
		if (expected) expect(expected.endsWith('Z')).toBe(true);
	}
	replay.seek(0, 0);
	expect(attributes.d).toBe(data);
});

it('rejects mismatched SVG stroke identities instead of revealing the wrong ink', () => {
	const root = { querySelectorAll: () => [strokeNode(1)] } as unknown as SVGSVGElement;
	expect(() => new ReplaySvg(root, { strokes: [{}] } as Replay)).toThrow('does not match');
});
