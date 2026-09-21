export type DebugRequest =
	| { kind: 'index' | 'document' }
	| { kind: 'entry'; entry: number }
	| { kind: 'bytes'; entry: number | null; offset: number; length: number }
	| { kind: 'page' | 'replay'; page: number }
	| { kind: 'background'; page: number; colorMode: 'auto' | 'light' | 'dark' }
	| { kind: 'layer'; page: number; layer: number }
	| { kind: 'object'; page: number; offset: number };
export type DebugQuery = <T>(request: DebugRequest) => Promise<T>;
export interface Index {
	entries: { index: number; name: string; size: number }[];
	pages: {
		index: number;
		visibleIndex: number | null;
		name: string;
		id: string;
	}[];
	sourceSize: number;
	zipLength: number;
}
export interface Box {
	x_min: number;
	y_min: number;
	x_max: number;
	y_max: number;
}
export interface Stroke {
	points: { x: number; y: number }[];
	pressures: number[];
	timestamps: number[];
	tilts: number[];
	orientations: number[];
	pen_width: number;
	color: { r: number; g: number; b: number } | null;
	bbox: Box;
}
export interface ReplayStroke {
	geometry: {
		points?: Stroke['points'];
		sample_ends?: number[];
		width: number;
		dot_radii?: number[];
		segment_widths: number[] | null;
		bounds: Box | null;
		color: string;
		opacity: number;
		profile: string | null;
		support: 'approximate' | 'reconstructed';
	};
	offset: number;
	milliseconds: boolean;
	stroke: Stroke;
}
export interface Replay {
	entry: number;
	width: number;
	height: number;

	strokes: ReplayStroke[];
	objects: { offset: number; bbox?: Box; type: unknown }[];
}
export interface Track {
	offset: number;
	start: number;
	end: number;
	times: number[];
	synthetic: boolean;
}
export function timeline(strokes: ReplayStroke[]): Track[] {
	let cursor = 0;
	return strokes.map(({ offset, stroke, milliseconds }) => {
		const ts = stroke.timestamps;
		const valid =
			milliseconds &&
			ts.length === stroke.points.length &&
			ts.every((t, i) => Number.isFinite(t) && (!i || t >= ts[i - 1]));
		const times = stroke.points.map((_, i) => (valid ? ts[i] - ts[0] : i * 16));
		const start = cursor;
		const end = start + (times.at(-1) ?? 0);
		cursor = end + 150;
		return { offset, start, end, times, synthetic: !valid };
	});
}
export function sampleAt(times: number[], elapsed: number): number {
	let lo = 0,
		hi = times.length;
	while (lo < hi) {
		const mid = (lo + hi) >>> 1;
		if (times[mid] <= elapsed) lo = mid + 1;
		else hi = mid;
	}
	return lo - 1;
}
export function hexRows(bytes: number[], offset: number): string {
	const rows = [];
	for (let i = 0; i < bytes.length; i += 16) {
		const row = bytes.slice(i, i + 16);
		rows.push(
			(offset + i).toString(16).padStart(8, '0') +
				'  ' +
				row
					.map((b) => b.toString(16).padStart(2, '0'))
					.join(' ')
					.padEnd(47) +
				'  ' +
				row
					.map((b) => (b >= 32 && b < 127 ? String.fromCharCode(b) : '.'))
					.join('')
		);
	}
	return rows.join('\n');
}

/** Transient overlay state for the existing document viewer. */
export interface DebugPreview {
	pageIndex: number;
	sourcePage: number;
	replay: Replay;
	tracks: Track[];
	position: number;
	selectedOffset: number | null;
	sampleIndex: number;
	onSelect: (offset: number) => void;
}
