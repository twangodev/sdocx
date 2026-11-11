import { sampleAt, timeline, type Replay, type ReplayStroke, type Stroke } from './model';

/** Local replay state. It never issues inspection requests. */
export class DebuggerPlayback {
	replay = $state.raw<Replay | null>(null);
	selectedOffset = $state<number | null>(null);
	selectedStroke = $state.raw<Stroke | null>(null);
	sampleIndex = $state(-1);
	playing = $state(false);
	position = $state(0);
	speed = $state(1);
	tracks = $derived(timeline(this.replay?.strokes ?? []));
	duration = $derived(this.tracks.at(-1)?.end ?? 0);

	constructor(private readonly onStroke: (stroke: ReplayStroke) => void) {}

	reset(): void {
		this.playing = false;
		this.replay = null;
		this.clearSelection();
		this.position = 0;
	}

	clearSelection(): void {
		this.selectedOffset = null;
		this.selectedStroke = null;
		this.sampleIndex = -1;
	}

	load(replay: Replay): void {
		this.replay = replay;
		this.position = this.duration;
	}

	select(offset: number | null, stroke: Stroke | null = null): void {
		this.playing = false;
		this.selectedOffset = offset;
		this.selectedStroke = stroke;
		this.sampleIndex = -1;
		const track = this.tracks.find((track) => track.offset === offset);
		if (track) this.position = track.end;
	}

	scrub(value: number): void {
		this.playing = false;
		this.position = value;
		this.syncSample();
	}

	advance(elapsed: number): void {
		this.position = Math.min(this.duration, this.position + elapsed * this.speed);
		this.syncSample();
		if (this.position >= this.duration) this.playing = false;
	}

	private syncSample(): void {
		const track = this.tracks.find((track) => this.position >= track.start && this.position <= track.end)
			?? this.tracks.findLast((track) => track.end <= this.position);
		if (!track) return;
		const item = this.replay?.strokes.find((stroke) => stroke.offset === track.offset);
		if (this.selectedOffset !== track.offset && item) this.onStroke(item);
		this.selectedOffset = track.offset;
		this.selectedStroke = item?.stroke ?? null;
		this.sampleIndex = sampleAt(track.times, this.position - track.start);
	}

	selectSample(index: number): void {
		this.playing = false;
		this.sampleIndex = index;
		const track = this.tracks.find((track) => track.offset === this.selectedOffset);
		if (track) this.position = track.start + track.times[index];
	}

	step(direction: number): number | undefined {
		this.playing = false;
		const current = this.tracks.findIndex((track) => track.offset === this.selectedOffset);
		const track = this.tracks[Math.max(0, Math.min(this.tracks.length - 1, current + direction))];
		if (track) this.position = track.end;
		return track?.offset;
	}
}
