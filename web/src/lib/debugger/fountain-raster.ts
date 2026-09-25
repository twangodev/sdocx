import type { ReplayStroke } from './model';
import { loadFountainInkRenderer } from '$converter/wasm-adapter';

/** The browser only supplies its pixel grid and displays Rust's RGBA output. */
export class FountainRaster {
	private canvas = document.createElement('canvas');

	constructor(private render: (input: string) => Uint8Array) {}

	draw(ctx: CanvasRenderingContext2D, row: ReplayStroke, last: number) {
		const { geometry, stroke } = row;
		const transform = ctx.getTransform();
		if (transform.b !== 0 || transform.c !== 0) {
			throw new Error('Ink viewport must use scale and translation');
		}
		const bytes = this.render(JSON.stringify({
			points: (geometry.points ?? stroke.points).slice(0, last + 1),
			dot_radii: geometry.dot_radii!.slice(0, last + 1),
			dot_directions: geometry.dot_directions?.slice(0, last + 1),
			fountain_shader: geometry.fountain_shader,
			color: ctx.strokeStyle,
			opacity: geometry.opacity,
			viewport: {
				width: ctx.canvas.width, height: ctx.canvas.height,
				scale_x: transform.a, scale_y: transform.d,
				offset_x: transform.e, offset_y: transform.f,
				subpixel_bits: 4
			}
		}));
		if (!bytes.length) return;
		const header = new DataView(bytes.buffer, bytes.byteOffset, 16);
		const [x, y, width, height] = [0, 4, 8, 12].map(i => header.getUint32(i, true));
		if (this.canvas.width !== width) this.canvas.width = width;
		if (this.canvas.height !== height) this.canvas.height = height;
		const rgba = new Uint8ClampedArray(bytes.subarray(16));
		this.canvas.getContext('2d')!.putImageData(new ImageData(rgba, width, height), 0, 0);
		ctx.save();
		ctx.resetTransform();
		ctx.drawImage(this.canvas, x, y);
		ctx.restore();
	}

	dispose() {
		this.canvas.width = this.canvas.height = 0;
	}
}

export async function loadFountainRaster(): Promise<() => FountainRaster> {
	const render = await loadFountainInkRenderer();
	return () => new FountainRaster(render);
}
