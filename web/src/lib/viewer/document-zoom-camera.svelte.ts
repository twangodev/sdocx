import { tick } from 'svelte';
import type { ZoomAnchor } from './wheel-zoom';
import { nextZoomStep, roundedZoom, type ZoomDirection } from './zoom';

interface ScrollAnchor {
	clientX: number;
	clientY: number;
	viewportX: number;
	viewportY: number;
	contentX: number;
	contentY: number;
	width: number;
	height: number;
	page?: {
		index: number;
		x: number;
		y: number;
	};
}

export class DocumentZoomCamera {
	scroller = $state<HTMLDivElement>();
	surface = $state<HTMLDivElement>();
	committedZoom = $state(100);
	pageFit = $state(true);
	gestureZoom = $state<number | null>(null);
	gestureOrigin = $state({ x: 0, y: 0 });

	private readonly selectedPage: () => number;
	private gestureBaseZoom = 100;
	private gestureAnchor: ScrollAnchor | undefined;
	private revision = 0;

	constructor(selectedPage: () => number) {
		this.selectedPage = selectedPage;
	}

	get visibleZoom(): number {
		return this.gestureZoom ?? this.committedZoom;
	}

	get visiblePageFit(): boolean {
		return this.pageFit && this.gestureZoom === null;
	}

	get gestureScale(): number {
		return this.gestureZoom === null ? 1 : this.gestureZoom / this.gestureBaseZoom;
	}

	reset(): void {
		this.revision += 1;
		this.resetGesture();
		this.committedZoom = 100;
		this.pageFit = true;
	}

	setZoom = async (zoom: number, anchor?: ZoomAnchor): Promise<void> => {
		const revision = ++this.revision;
		const previous = this.captureAnchor(anchor);
		this.resetGesture();
		this.pageFit = false;
		this.committedZoom = roundedZoom(zoom);
		if (!previous) return;
		await tick();
		if (revision === this.revision) this.restoreAnchor(previous);
	};

	stepZoom = (direction: ZoomDirection, anchor?: ZoomAnchor): void => {
		const currentZoom = this.gestureZoom ?? (this.pageFit ? 100 : this.committedZoom);
		void this.setZoom(nextZoomStep(currentZoom, direction), anchor);
	};

	fitWidth = (): void => {
		void this.setZoom(100);
	};

	fitPage = (): void => {
		this.revision += 1;
		this.resetGesture();
		this.pageFit = true;
	};

	updateGesture = (factor: number, anchor: ZoomAnchor): void => {
		if (!this.surface || !this.scroller || !Number.isFinite(factor) || factor <= 0) return;
		if (this.gestureZoom === null) {
			const bounds = this.surface.getBoundingClientRect();
			this.gestureBaseZoom = this.pageFit ? this.currentFitPageZoom() : this.committedZoom;
			this.gestureZoom = this.gestureBaseZoom;
			this.gestureOrigin = {
				x: anchor.clientX - bounds.left,
				y: anchor.clientY - bounds.top
			};
			this.gestureAnchor = this.captureAnchor(anchor);
		}

		this.gestureZoom = roundedZoom(this.gestureZoom * factor);
	};

	finishGesture = async (): Promise<void> => {
		if (this.gestureZoom === null) return;
		const revision = ++this.revision;
		const targetZoom = this.gestureZoom;
		const previous = this.gestureAnchor;
		this.pageFit = false;
		this.committedZoom = targetZoom;
		this.resetGesture();
		if (!previous) return;
		await tick();
		if (revision === this.revision) this.restoreAnchor(previous);
	};

	private currentFitPageZoom(): number {
		if (!this.surface) return this.committedZoom;
		const selected =
			this.surface.querySelector<HTMLImageElement>(
				`[data-page-index="${this.selectedPage()}"] img`
			) ?? this.surface.querySelector<HTMLImageElement>('img');
		if (!selected) return this.committedZoom;
		const surfaceWidth = this.surface.getBoundingClientRect().width;
		if (surfaceWidth <= 0) return this.committedZoom;
		return roundedZoom((selected.getBoundingClientRect().width / surfaceWidth) * 100);
	}

	private captureAnchor(anchor?: ZoomAnchor): ScrollAnchor | undefined {
		if (!this.scroller) return undefined;
		const bounds = this.scroller.getBoundingClientRect();
		const viewportX = anchor ? anchor.clientX - bounds.left : this.scroller.clientWidth / 2;
		const viewportY = anchor ? anchor.clientY - bounds.top : this.scroller.clientHeight / 2;
		const clientX = bounds.left + viewportX;
		const clientY = bounds.top + viewportY;
		const page = this.pageAt(clientY);
		return {
			clientX,
			clientY,
			viewportX,
			viewportY,
			contentX: this.scroller.scrollLeft + viewportX,
			contentY: this.scroller.scrollTop + viewportY,
			width: this.scroller.scrollWidth,
			height: this.scroller.scrollHeight,
			...(page
				? {
						page: {
							index: Number(page.element.dataset.pageIndex),
							x: (clientX - page.bounds.left) / page.bounds.width,
							y: (clientY - page.bounds.top) / page.bounds.height
						}
					}
				: {})
		};
	}

	private restoreAnchor(previous: ScrollAnchor): void {
		if (!this.scroller) return;
		if (previous.page) {
			const page = this.scroller.querySelector<HTMLElement>(
				`[data-page-index="${previous.page.index}"]`
			);
			if (page) {
				const bounds = page.getBoundingClientRect();
				this.scroller.scrollLeft += bounds.left + bounds.width * previous.page.x - previous.clientX;
				this.scroller.scrollTop += bounds.top + bounds.height * previous.page.y - previous.clientY;
				return;
			}
		}

		const widthScale = previous.width > 0 ? this.scroller.scrollWidth / previous.width : 1;
		const heightScale = previous.height > 0 ? this.scroller.scrollHeight / previous.height : 1;
		this.scroller.scrollLeft = previous.contentX * widthScale - previous.viewportX;
		this.scroller.scrollTop = previous.contentY * heightScale - previous.viewportY;
	}

	private pageAt(clientY: number): { element: HTMLElement; bounds: DOMRect } | undefined {
		if (!this.surface) return undefined;
		for (const element of this.surface.querySelectorAll<HTMLElement>('[data-page-index]')) {
			const bounds = element.getBoundingClientRect();
			if (bounds.top <= clientY && bounds.bottom >= clientY && bounds.width > 0 && bounds.height > 0) {
				return { element, bounds };
			}
		}
		return undefined;
	}

	private resetGesture(): void {
		this.gestureZoom = null;
		this.gestureBaseZoom = 100;
		this.gestureAnchor = undefined;
	}
}
