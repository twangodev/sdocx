import type { CorpusFixture } from './manifest';
import type { RenderedDocument, RenderedPage } from './commit-protocol';
import { runStructuralChecks, type StructuralCheck } from './structure';

export type PageDiffStatus = 'identical' | 'changed' | 'left-only' | 'right-only';

export interface CommitPageDiff {
	pageIndex: number;
	status: PageDiffStatus;
	left?: RenderedPage;
	right?: RenderedPage;
	leftUrl?: string;
	rightUrl?: string;
}

export interface StructuralDiff {
	id: string;
	label: string;
	left: string | number | boolean | undefined;
	right: string | number | boolean | undefined;
	changed: boolean;
}

export interface DocumentDiff {
	changed: boolean;
	changedPageCount: number;
	leftChecks: StructuralCheck[];
	rightChecks: StructuralCheck[];
	structuralDiffs: StructuralDiff[];
	pages: CommitPageDiff[];
}

function sameValue(left: unknown, right: unknown): boolean {
	return Object.is(left, right);
}

export function compareRenderedDocuments(
	fixture: CorpusFixture,
	left: RenderedDocument,
	right: RenderedDocument
): DocumentDiff {
	const pageCount = Math.max(left.pages.length, right.pages.length);
	const pages = Array.from({ length: pageCount }, (_, pageIndex): CommitPageDiff => {
		const leftPage = left.pages[pageIndex];
		const rightPage = right.pages[pageIndex];
		const status: PageDiffStatus =
			leftPage && rightPage
				? leftPage.svg === rightPage.svg
					? 'identical'
					: 'changed'
				: leftPage
					? 'left-only'
					: 'right-only';
		return { pageIndex, status, left: leftPage, right: rightPage };
	});

	const leftChecks = runStructuralChecks(fixture, left.inspection, left.pageCount).checks;
	const rightChecks = runStructuralChecks(fixture, right.inspection, right.pageCount).checks;
	const checkIds = new Set([...leftChecks.map((check) => check.id), ...rightChecks.map((check) => check.id)]);
	const structuralDiffs = Array.from(checkIds, (id): StructuralDiff => {
		const leftCheck = leftChecks.find((check) => check.id === id);
		const rightCheck = rightChecks.find((check) => check.id === id);
		return {
			id,
			label: leftCheck?.label ?? rightCheck?.label ?? id,
			left: leftCheck?.actual,
			right: rightCheck?.actual,
			changed:
				!sameValue(leftCheck?.actual, rightCheck?.actual) ||
				!sameValue(leftCheck?.passed, rightCheck?.passed)
		};
	});
	const changedPageCount = pages.filter((page) => page.status !== 'identical').length;
	return {
		changed: changedPageCount > 0 || structuralDiffs.some((item) => item.changed),
		changedPageCount,
		leftChecks,
		rightChecks,
		structuralDiffs,
		pages
	};
}

function pageUrl(page?: RenderedPage): string | undefined {
	return page
		? URL.createObjectURL(new Blob([page.svg], { type: 'image/svg+xml;charset=utf-8' }))
		: undefined;
}

export function attachPageUrls(diff: DocumentDiff): DocumentDiff {
	return {
		...diff,
		pages: diff.pages.map((page) => ({
			...page,
			leftUrl: pageUrl(page.left),
			rightUrl: pageUrl(page.right)
		}))
	};
}

export function disposeDocumentDiff(diff?: DocumentDiff): void {
	if (!diff) return;
	for (const page of diff.pages) {
		if (page.leftUrl) URL.revokeObjectURL(page.leftUrl);
		if (page.rightUrl) URL.revokeObjectURL(page.rightUrl);
	}
}
