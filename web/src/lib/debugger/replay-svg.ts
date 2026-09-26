import type { Replay } from './model';

class StrokeReveal {
	private parts: SVGElement[];
	private paths: { node: SVGElement; attribute: string; value: string; lengths: number[] }[];
	private count = Infinity;

	constructor(node: SVGElement) {
		this.parts = [...node.querySelectorAll<SVGElement>('[data-replay-part]')];
		this.paths = [...node.querySelectorAll<SVGElement>('[data-replay-lengths]')].map(node => {
			const attribute = node.tagName === 'polyline' ? 'points' : 'd';
			return {
				node, attribute, value: node.getAttribute(attribute)!,
				lengths: node.dataset.replayLengths!.split(',').map(Number)
			};
		});
	}

	show(count: number) {
		if (count === this.count) return;
		this.count = count;
		for (const node of this.parts) {
			node.style.display = Number(node.dataset.replayPart) <= count ? '' : 'none';
		}
		for (const { node, attribute, value, lengths } of this.paths) {
			const end = count >= lengths.length ? value.length : (lengths[count - 1] ?? 0);
			node.setAttribute(attribute, value.slice(0, end));
		}
	}
}

export class ReplaySvg {
	private strokes: SVGElement[];
	private reveals = new Map<number, StrokeReveal>();
	private active = -1;

	constructor(root: SVGSVGElement, private replay: Replay) {
		this.strokes = [...root.querySelectorAll<SVGElement>('[data-replay-stroke]')]
			.sort((a, b) => Number(a.dataset.replayStroke) - Number(b.dataset.replayStroke));
		if (this.strokes.length !== replay.strokes.length ||
			this.strokes.some((node, i) => Number(node.dataset.replayStroke) !== i)) {
			throw new Error('Replay SVG does not match the stroke timeline');
		}
	}

	seek(complete: number, sample: number) {
		const active = complete + 1;
		if (active !== this.active) this.reveals.get(this.active)?.show(Infinity);
		this.active = active;
		for (let i = 0; i < this.strokes.length; i++) {
			const display = i <= complete || (i === active && sample >= 0) ? '' : 'none';
			if (this.strokes[i].style.display !== display) this.strokes[i].style.display = display;
		}
		if (sample < 0 || active >= this.strokes.length) return;
		let reveal = this.reveals.get(active);
		if (!reveal) {
			reveal = new StrokeReveal(this.strokes[active]);
			this.reveals.set(active, reveal);
		}
		const { stroke, geometry } = this.replay.strokes[active];
		const count = geometry.sample_ends?.[sample] ?? Math.min(sample + 1, stroke.points.length);
		reveal.show(count);
	}
}
