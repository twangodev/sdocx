import type { FixtureStatus } from './runner';

export type StatusTone = 'neutral' | 'active' | 'success' | 'danger';

export function statusTone(status: FixtureStatus): StatusTone {
	if (status === 'passed') return 'success';
	if (status === 'failed' || status === 'cancelled') return 'danger';
	if (status === 'queued') return 'neutral';
	return 'active';
}

export function clampPageIndex(pageIndex: number, pageCount: number): number {
	if (pageCount <= 0) return 0;
	return Math.max(0, Math.min(Math.trunc(pageIndex), pageCount - 1));
}
