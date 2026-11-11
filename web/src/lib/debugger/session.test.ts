import { describe, expect, it, vi } from 'vitest';
import { DebuggerSession } from './session.svelte';
import { DebuggerPlayback } from './playback.svelte';
import type { DebugQuery, DebugRequest, Index, Replay } from './model';

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (cause: unknown) => void;
	const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
	return { promise, resolve, reject };
}
function harness() {
	const requests: { request: DebugRequest; result: ReturnType<typeof deferred<unknown>> }[] = [];
	const query: DebugQuery = <T>(request: DebugRequest) => {
		const result = deferred<unknown>();
		requests.push({ request, result });
		return result.promise as Promise<T>;
	};
	const navigate = vi.fn();
	return { session: new DebuggerSession(query, navigate, vi.fn()), requests, navigate };
}
function replay(offset = 17, timestamps = [0, 10, 30]): Replay {
	return { entry: 1, width: 100, height: 100, objects: [], strokes: [{
		offset, milliseconds: true,
		geometry: { width: 1, segment_widths: null, bounds: null, color: '#111', opacity: 1,
			profile: 'test', support: 'reconstructed' },
		stroke: { points: timestamps.map((_, x) => ({ x, y: x })), timestamps,
			pressures: [], tilts: [], orientations: [], pen_width: 2, color: null,
			bbox: { x_min: 0, y_min: 0, x_max: 10, y_max: 10 } }
	}] };
}
const index: Index = { entries: [], sourceSize: 100, zipLength: 90,
	pages: [0, 1].map((index) => ({ index, visibleIndex: index, name: `page${index}`, id: `${index}` })) };
async function settle() { await Promise.resolve(); await Promise.resolve(); }

describe('debugger inspection transitions', () => {
	it('accepts only the latest page replay and retains busy until it responds', async () => {
		const { session, requests, navigate } = harness();
		session.index = index;
		const first = session.loadPage(0), second = session.loadPage(1);
		requests[0].result.resolve(replay(10)); await first;
		expect(session.playback.replay).toBeNull(); expect(session.busy).toBe(true);
		requests[1].result.resolve(replay(20)); await second;
		expect(session.playback.replay?.strokes[0].offset).toBe(20);
		expect(session.busy).toBe(false); expect(navigate.mock.calls).toEqual([[0], [1]]);
	});
	it('shares one selection boundary between tree and overlay requests', async () => {
		const { session, requests } = harness();
		const tree = session.choose({ kind: 'object', page: 0, offset: 10 });
		const overlay = session.pick(20);
		requests[1].result.resolve({ offset: 20, name: 'new' }); await overlay;
		requests[0].result.resolve({ offset: 10, name: 'old' });
		expect(await tree).toBeNull(); expect(session.properties.name).toBe('new');
		expect(session.playback.selectedOffset).toBe(20);
	});
	it.each([1, -1])('keeps the selected navigation anchor during repeated pending steps (%s)', async (direction) => {
		const { session, requests } = harness();
		const page = replay();
		page.strokes.push(replay(25).strokes[0]);
		session.playback.load(page);
		session.playback.select(direction === 1 ? 17 : 25);
		session.step(direction);
		session.step(direction);
		const target = direction === 1 ? 25 : 17;
		expect(requests.map(({ request }) => request)).toEqual([
			{ kind: 'object', page: 0, offset: target },
			{ kind: 'object', page: 0, offset: target }
		]);
		// An old selection's debounce must not supersede the pending step.
		session.inspectSelected();
		expect(requests).toHaveLength(2);
		requests[0].result.resolve({ offset: target, name: 'old step' });
		await settle();
		expect(session.playback.selectedOffset).toBe(direction === 1 ? 17 : 25);
		requests[1].result.resolve({ offset: target, name: 'latest step' });
		await settle();
		expect(session.playback.selectedOffset).toBe(target);
		expect(session.properties.name).toBe('latest step');
	});
	it('invalidates selection and hex work when the page changes', async () => {
		const { session, requests } = harness(); session.entry = 3;
		const bytes = session.readHex(4096), selection = session.pick(10), page = session.loadPage(1);
		requests[0].result.resolve({ bytes: [65], offset: 4096, total: 5000 });
		requests[1].result.resolve({ entry: 3, offset: 10 });
		await Promise.all([bytes, selection]);
		expect(session.properties).toEqual({}); expect(session.hex).toBe('');
		expect(session.entry).toBeNull(); expect(session.playback.selectedOffset).toBeNull();
		expect(requests).toHaveLength(3);
		requests[2].result.resolve(replay()); await page;
	});
	it('invalidates a debounced inspection when another entry is chosen', async () => {
		const { session, requests } = harness(); session.playback.select(17);
		session.inspectSelected();
		const entry = session.choose({ kind: 'entry', entry: 8 });
		requests[1].result.resolve({ entry: 8, name: 'entry eight' }); await entry;
		requests[0].result.resolve({ entry: 1, offset: 17, name: 'stale stroke' }); await settle();
		expect(session.properties.name).toBe('entry eight'); expect(session.entry).toBe(8);
		expect(session.playback.selectedOffset).toBeNull(); expect(requests).toHaveLength(3);
		expect(requests[2].request).toMatchObject({ kind: 'bytes', entry: 8 });
	});
	it('invalidates pending selection when original file bytes are selected', async () => {
		const { session, requests } = harness(); session.index = index;
		const object = session.pick(17); session.selectOriginal();
		requests[1].result.resolve({ bytes: [66], offset: 0, total: 100 }); await settle();
		requests[0].result.resolve({ entry: 2, offset: 17 }); await object;
		expect(session.entry).toBeNull(); expect(session.hex).toContain('42');
		expect(session.properties.sourceSize).toBe(100);
	});
	it('accepts only the latest hex response and ignores stale failures', async () => {
		const { session, requests } = harness();
		const old = session.readHex(0), current = session.readHex(4096);
		requests[1].result.resolve({ bytes: [66], offset: 4096, total: 5000 }); await current;
		requests[0].result.reject(new Error('old error')); await old;
		expect(session.hex).toContain('00001000'); expect(session.error).toBe('');
	});
	it('preserves a cross-page tree selection while replay loads, then adds geometry', async () => {
		const { session, requests } = harness();
		const selection = session.choose({ kind: 'object', page: 1, offset: 17 });
		requests[0].result.resolve({ offset: 17, stroke: replay().strokes[0].stroke }); await selection;
		expect(session.page).toBe(1); expect(session.playback.selectedOffset).toBe(17);
		expect(session.properties.rendering).toBeUndefined();
		requests[1].result.resolve(replay()); await settle();
		expect(session.playback.selectedOffset).toBe(17);
		expect(session.properties.rendering).toEqual({ profile: 'test', support: 'reconstructed' });
	});
	it('cancels effect-owned inspection and ignores all responses after disposal', async () => {
		const { session, requests } = harness();
		const startup = session.start(), page = session.loadPage(0);
		session.playback.select(17); const cancel = session.inspectSelected(); cancel();
		requests[2].result.resolve({ name: 'cancelled' }); await settle();
		expect(session.properties).toEqual({});
		const bytes = session.readHex(), selection = session.pick(22); session.destroy();
		requests[0].result.resolve(index); requests[1].result.resolve(replay());
		requests[3].result.resolve({ bytes: [65], offset: 0, total: 10 });
		requests[4].result.reject(new Error('disposed'));
		await Promise.all([startup, page, bytes, selection]);
		expect(session.index).toBeNull(); expect(session.playback.replay).toBeNull();
		expect(session.hex).toBe(''); expect(session.error).toBe('');
	});
});
describe('debugger replay transitions', () => {
	it('scrubs locally and synchronizes sample, step, speed and end-of-playback transitions', () => {
		const onStroke = vi.fn(), playback = new DebuggerPlayback(onStroke), page = replay();
		page.strokes.push(replay(25).strokes[0]); playback.load(page);
		expect(playback.duration).toBe(210); playback.scrub(10);
		expect(playback.selectedOffset).toBe(17); expect(playback.sampleIndex).toBe(1);
		playback.scrub(20); expect(onStroke).toHaveBeenCalledTimes(1);
		playback.selectSample(2); expect(playback.position).toBe(30);
		expect(playback.step(1)).toBe(25); playback.select(25, page.strokes[1].stroke);
		expect(playback.position).toBe(210); playback.scrub(180);
		expect(playback.sampleIndex).toBe(0); playback.speed = 2; playback.playing = true;
		playback.advance(20); expect(playback.position).toBe(210); expect(playback.playing).toBe(false);
		playback.reset(); expect(playback.replay).toBeNull(); expect(playback.selectedStroke).toBeNull();
		expect(playback.sampleIndex).toBe(-1);
	});
	it('labels malformed millisecond channels with synthetic timing', () => {
		const { session } = harness(); session.playback.load(replay(17, [5, 2, 8]));
		session.playback.scrub(16); expect(session.properties.timing).toBe('Synthetic sample timing');
		expect(session.playback.sampleIndex).toBe(1);
	});
});
