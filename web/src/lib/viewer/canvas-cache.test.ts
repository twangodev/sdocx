import { expect, it, vi } from 'vitest';
import { CanvasCache } from './canvas-cache';
const entry = () => ({
	canvas: { width: 10, height: 10 } as HTMLCanvasElement
});
it('evicts the least recently used surface within its byte budget and frees backing stores', () => {
	const cache = new CanvasCache<string, ReturnType<typeof entry>>(800);
	const a = entry(),
		b = entry(),
		c = entry(),
		evicted = vi.fn();
	cache.set('a', a);
	cache.set('b', b, evicted);
	cache.get('a');
	cache.set('c', c);
	expect(cache.get('b')).toBeUndefined();
	expect(b.canvas.width).toBe(0);
	expect(evicted).toHaveBeenCalledOnce();
	expect(cache.bytes).toBe(800);
	cache.clear();
	expect(cache.bytes).toBe(0);
	expect(a.canvas.width + c.canvas.height).toBe(0);
});
it('enforces the gesture page count and rejects oversized surfaces without evicting valid entries', () => {
	const cache = new CanvasCache<string, ReturnType<typeof entry>>(1600, 1);
	const a = entry(),
		b = entry();
	cache.set('a', a);
	cache.set('b', b);
	expect(a.canvas.width).toBe(0);
	const huge = { canvas: { width: 100, height: 100 } as HTMLCanvasElement };
	expect(cache.set('huge', huge)).toBe(false);
	expect(cache.get('b')).toBe(b);
});
