import type { Replay } from './model';
import { CanvasCache } from '$lib/viewer/canvas-cache';
import type { PageRegion } from '$lib/viewer/page-region';

const BATCH_SIZE = 32;
const TILE_SIZE = 512;
interface Bounds {
	left: number;
	top: number;
	right: number;
	bottom: number;
}
interface Tile {
	canvas: HTMLCanvasElement;
	x: number;
	y: number;
}
function intersects(a: Bounds, b: Bounds) {
	return (
		a.left <= b.right &&
		a.right >= b.left &&
		a.top <= b.bottom &&
		a.bottom >= b.top
	);
}
function emptyBounds(): Bounds {
	return { left: Infinity, top: Infinity, right: -Infinity, bottom: -Infinity };
}
function include(a: Bounds, b: Bounds) {
	a.left = Math.min(a.left, b.left);
	a.top = Math.min(a.top, b.top);
	a.right = Math.max(a.right, b.right);
	a.bottom = Math.max(a.bottom, b.bottom);
}

/** Timeline composition and spatial ink tiles; independent of the viewer camera and DOM. */
export class ReplayRaster {
	private tiles = new CanvasCache<string, Tile>(48 * 1024 * 1024);
	private bounds: Bounds[] = [];
	private batches: Bounds[] = [];
	private base: HTMLCanvasElement | undefined;
	private regionKey = '';
	private completed = -1;

	constructor(
		private replay: Replay,
		private defaultInk: string
	) {
		for (let i = 0; i < replay.strokes.length; i++) {
			const { stroke, paint } = replay.strokes[i];
			let width = paint.width;
			for (const value of paint.segment_widths ?? [])
				width = Math.max(width, value);
			const bounds = emptyBounds();
			for (const point of stroke.points)
				include(bounds, {
					left: point.x - width / 2,
					top: point.y - width / 2,
					right: point.x + width / 2,
					bottom: point.y + width / 2
				});
			this.bounds.push(bounds);
			const batch = Math.floor(i / BATCH_SIZE);
			this.batches[batch] ??= emptyBounds();
			include(this.batches[batch], bounds);
		}
	}

	/** Paint only the visible region. False means cold work continues next frame. */
	paint(
		ctx: CanvasRenderingContext2D,
		region: PageRegion,
		complete: number,
		sample: number
	): boolean {
		this.base ??= document.createElement('canvas');
		const key = `${region.scale}:${region.x}:${region.y}:${region.width}:${region.height}`;
		const cache = this.base.getContext('2d')!;
		if (key !== this.regionKey || complete < this.completed) {
			if (this.base.width !== region.width) this.base.width = region.width;
			if (this.base.height !== region.height) this.base.height = region.height;
			cache.resetTransform();
			cache.clearRect(0, 0, region.width, region.height);
			this.completed = -1;
			this.regionKey = key;
		}
		this.completed =
			this.draw(
				cache,
				region,
				this.completed + 1,
				complete + 1,
				performance.now() + 6
			) - 1;
		if (this.completed < complete) return false;
		ctx.resetTransform();
		ctx.clearRect(0, 0, region.width, region.height);
		ctx.drawImage(this.base, 0, 0);
		if (sample >= 0 && complete + 1 < this.replay.strokes.length) {
			ctx.setTransform(region.scale, 0, 0, region.scale, -region.x, -region.y);
			this.drawStroke(ctx, complete + 1, sample);
		}
		return true;
	}
	drawStroke(ctx: CanvasRenderingContext2D, index: number, last: number) {
		if (last < 1) return;
		const { stroke, paint } = this.replay.strokes[index];
		ctx.strokeStyle = stroke.color ? paint.color : this.defaultInk;
		ctx.lineCap = 'round';
		ctx.lineJoin = 'round';
		if (paint.segment_widths) {
			for (let j = 1; j <= last; j++) {
				const a = stroke.points[j - 1],
					b = stroke.points[j];
				ctx.lineWidth = paint.segment_widths[j - 1];
				ctx.beginPath();
				ctx.moveTo(a.x, a.y);
				ctx.lineTo(b.x, b.y);
				ctx.stroke();
			}
		} else {
			ctx.lineWidth = paint.width;
			ctx.beginPath();
			for (let j = 0; j <= last; j++) {
				const p = stroke.points[j];
				if (j) ctx.lineTo(p.x, p.y);
				else ctx.moveTo(p.x, p.y);
			}
			ctx.stroke();
		}
	}

	private draw(
		ctx: CanvasRenderingContext2D,
		region: PageRegion,
		start: number,
		end: number,
		deadline: number
	) {
		const view = {
			left: (region.x - 2) / region.scale,
			top: (region.y - 2) / region.scale,
			right: (region.x + region.width + 2) / region.scale,
			bottom: (region.y + region.height + 2) / region.scale
		};
		while (start < end) {
			if (start % BATCH_SIZE === 0 && start + BATCH_SIZE <= end) {
				const bounds = this.batches[start / BATCH_SIZE];
				if (intersects(bounds, view)) {
					const left = Math.max(
						region.x,
						Math.floor(bounds.left * region.scale) - 2
					);
					const top = Math.max(
						region.y,
						Math.floor(bounds.top * region.scale) - 2
					);
					const right = Math.min(
						region.x + region.width,
						Math.ceil(bounds.right * region.scale) + 2
					);
					const bottom = Math.min(
						region.y + region.height,
						Math.ceil(bounds.bottom * region.scale) + 2
					);
					ctx.resetTransform();
					for (
						let y = Math.floor(top / TILE_SIZE) * TILE_SIZE;
						y < bottom;
						y += TILE_SIZE
					)
						for (
							let x = Math.floor(left / TILE_SIZE) * TILE_SIZE;
							x < right;
							x += TILE_SIZE
						) {
							const tile = this.tile(start, x, y, region.scale);
							ctx.drawImage(tile.canvas, tile.x - region.x, tile.y - region.y);
						}
				}
				start += BATCH_SIZE;
			} else {
				if (intersects(this.bounds[start], view)) {
					ctx.setTransform(
						region.scale,
						0,
						0,
						region.scale,
						-region.x,
						-region.y
					);
					this.drawStroke(
						ctx,
						start,
						this.replay.strokes[start].stroke.points.length - 1
					);
				}
				start++;
			}
			if (performance.now() >= deadline) break;
		}
		return start;
	}

	private tile(
		start: number,
		tileX: number,
		tileY: number,
		scale: number
	): Tile {
		const key = `${scale}:${tileX}:${tileY}:${start}`;
		const hit = this.tiles.get(key);
		if (hit) return hit;
		const bounds = this.batches[start / BATCH_SIZE];
		const x = Math.max(tileX, Math.floor(bounds.left * scale) - 2);
		const y = Math.max(tileY, Math.floor(bounds.top * scale) - 2);
		const width = Math.max(
			1,
			Math.min(tileX + TILE_SIZE, Math.ceil(bounds.right * scale) + 2) - x
		);
		const height = Math.max(
			1,
			Math.min(tileY + TILE_SIZE, Math.ceil(bounds.bottom * scale) + 2) - y
		);
		const canvas = document.createElement('canvas');
		canvas.width = width;
		canvas.height = height;
		const ctx = canvas.getContext('2d')!;
		ctx.setTransform(scale, 0, 0, scale, -x, -y);
		const view = {
			left: (x - 2) / scale,
			top: (y - 2) / scale,
			right: (x + width + 2) / scale,
			bottom: (y + height + 2) / scale
		};
		for (let i = start; i < start + BATCH_SIZE; i++)
			if (intersects(this.bounds[i], view))
				this.drawStroke(
					ctx,
					i,
					this.replay.strokes[i].stroke.points.length - 1
				);
		const tile = { canvas, x, y };
		this.tiles.set(key, tile);
		return tile;
	}

	dispose() {
		this.tiles.clear();
		if (this.base) this.base.width = this.base.height = 0;
		this.base = undefined;
	}
}
