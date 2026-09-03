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
		target: 'image' | 'page';
		imageIndex?: number;
		x: number;
		y: number;
	};
}

export class DocumentZoomCamera {
	scroller = $state<HTMLDivElement>();
	surface = $state<HTMLDivElement>();
	committedZoom = $state(100);
	pageFit = $state(true);
	panX = $state(0);
	gestureZoom = $state<number | null>(null);
	gestureOrigin = $state({ x: 0, y: 0 });
	recentering = $state(false);

	private readonly selectedPage: () => number;
	private gestureBaseZoom = 100;
	private gestureAnchor: ScrollAnchor | undefined;
	private recenterFrame = 0;
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

	get surfaceTransform(): string | undefined {
		const translation = `translate3d(${this.panX}px, 0, 0)`;
		if (this.gestureZoom === null) return this.panX === 0 ? undefined : translation;
		return `${translation} scale(${this.gestureScale})`;
	}

	reset(): void {
		this.cancelRecentering();
		this.revision += 1;
		this.resetGesture();
		this.committedZoom = 100;
		this.pageFit = true;
		this.panX = 0;
	}

	setZoom = async (
		zoom: number,
		anchor?: ZoomAnchor,
		centerHorizontally = false
	): Promise<void> => {
		this.cancelRecentering();
		const revision = ++this.revision;
		const previous = this.captureAnchor(anchor);
		this.resetGesture();
		this.pageFit = false;
		if (centerHorizontally) this.panX = 0;
		this.committedZoom = roundedZoom(zoom);
		if (!previous) return;
		await tick();
		if (revision === this.revision) this.restoreAnchor(previous, !centerHorizontally);
	};

	stepZoom = (direction: ZoomDirection, anchor?: ZoomAnchor): void => {
		const currentZoom = this.gestureZoom ?? (this.pageFit ? 100 : this.committedZoom);
		void this.setZoom(nextZoomStep(currentZoom, direction), anchor);
	};

	fitWidth = (): void => {
		void this.setZoom(100, undefined, true);
	};

	fitPage = (): void => {
		this.cancelRecentering();
		this.revision += 1;
		this.resetGesture();
		this.pageFit = true;
		this.panX = 0;
	};

	scrollToPage = (pageIndex: number): void => {
		const page = this.scroller?.querySelector<HTMLElement>(`[data-page-index="${pageIndex}"]`);
		page?.scrollIntoView({ behavior: 'auto', block: 'start' });
	};

	fitSelectedPage = (): void => {
		const selectedPage = this.selectedPage();
		this.fitPage();
		requestAnimationFrame(() => this.scrollToPage(selectedPage));
	};

	updateGesture = (factor: number, anchor: ZoomAnchor): void => {
		if (!this.surface || !this.scroller || !Number.isFinite(factor) || factor <= 0) return;
		this.cancelRecentering();
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
		this.cancelRecentering();
		const revision = ++this.revision;
		const targetZoom = this.gestureZoom;
		const previous = this.gestureAnchor;
		this.pageFit = false;
		this.committedZoom = targetZoom;
		this.resetGesture();
		if (!previous) return;
		await tick();
		if (revision === this.revision) {
			this.restoreAnchor(previous);
			this.scheduleRecentering();
		}
	};

	private currentFitPageZoom(): number {
		if (!this.surface) return this.committedZoom;
		const selectedPage = this.surface.querySelector<HTMLElement>(
			`[data-page-index="${this.selectedPage()}"]`
		);
		const selected =
			selectedPage?.querySelector<HTMLElement>('[data-page-zoom-target]') ??
			selectedPage?.querySelector<HTMLElement>('img') ??
			this.surface.querySelector<HTMLElement>('[data-page-zoom-target], img');
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
		const target = page ? this.anchorTarget(page, clientX, clientY) : undefined;
		return {
			clientX,
			clientY,
			viewportX,
			viewportY,
			contentX: this.scroller.scrollLeft + viewportX,
			contentY: this.scroller.scrollTop + viewportY,
			width: this.scroller.scrollWidth,
			height: this.scroller.scrollHeight,
			...(page && target
				? {
						page: {
							index: Number(page.element.dataset.pageIndex),
							target: target.kind,
							...(target.imageIndex === undefined ? {} : { imageIndex: target.imageIndex }),
							x: (clientX - target.bounds.left) / target.bounds.width,
							y: (clientY - target.bounds.top) / target.bounds.height
						}
					}
				: {})
		};
	}

	private restoreAnchor(previous: ScrollAnchor, preserveHorizontal = true): void {
		if (!this.scroller) return;
		if (previous.page) {
			const page = this.scroller.querySelector<HTMLElement>(
				`[data-page-index="${previous.page.index}"]`
			);
			if (page) {
				const target =
					previous.page.target === 'image'
						? page.querySelectorAll<HTMLElement>('img')[previous.page.imageIndex ?? 0]
						: page;
				const bounds = target?.getBoundingClientRect();
				if (!bounds) return;
				if (preserveHorizontal) {
					const deltaX = bounds.left + bounds.width * previous.page.x - previous.clientX;
					const previousScrollLeft = this.scroller.scrollLeft;
					this.scroller.scrollLeft += deltaX;
					const scrolledX = this.scroller.scrollLeft - previousScrollLeft;
					this.panX = Math.round((this.panX - (deltaX - scrolledX)) * 10) / 10;
				}
				this.scroller.scrollTop +=
					bounds.top + bounds.height * previous.page.y - previous.clientY;
				return;
			}
		}

		const widthScale = previous.width > 0 ? this.scroller.scrollWidth / previous.width : 1;
		const heightScale = previous.height > 0 ? this.scroller.scrollHeight / previous.height : 1;
		if (preserveHorizontal) {
			this.scroller.scrollLeft = previous.contentX * widthScale - previous.viewportX;
		}
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

	private anchorTarget(
		page: { element: HTMLElement; bounds: DOMRect },
		clientX: number,
		clientY: number
	): { kind: 'image' | 'page'; bounds: DOMRect; imageIndex?: number } {
		for (const [imageIndex, image] of page.element.querySelectorAll<HTMLElement>('img').entries()) {
			const bounds = image.getBoundingClientRect();
			if (
				bounds.left <= clientX &&
				bounds.right >= clientX &&
				bounds.top <= clientY &&
				bounds.bottom >= clientY
			) {
				return { kind: 'image', bounds, imageIndex };
			}
		}
		return { kind: 'page', bounds: page.bounds };
	}

	private scheduleRecentering(): void {
		if (!this.surface || !this.scroller || Math.abs(this.panX) < 0.1) return;
		if (this.surface.getBoundingClientRect().width > this.scroller.clientWidth) return;
		if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
			this.panX = 0;
			return;
		}

		const startPan = this.panX;
		let startTime: number | undefined;
		this.recentering = true;
		const animate = (time: number) => {
			if (startTime === undefined) {
				startTime = time;
				this.recenterFrame = requestAnimationFrame(animate);
				return;
			}
			const progress = Math.min(1, (time - startTime) / 220);
			const eased = 1 - (1 - progress) ** 3;
			this.panX = Math.round(startPan * (1 - eased) * 10) / 10;
			if (progress < 1) {
				this.recenterFrame = requestAnimationFrame(animate);
				return;
			}
			this.panX = 0;
			this.recenterFrame = 0;
			this.recentering = false;
		};
		this.recenterFrame = requestAnimationFrame(animate);
	}

	private cancelRecentering(): void {
		if (this.recenterFrame) cancelAnimationFrame(this.recenterFrame);
		this.recenterFrame = 0;
		this.recentering = false;
	}

	private resetGesture(): void {
		this.gestureZoom = null;
		this.gestureBaseZoom = 100;
		this.gestureAnchor = undefined;
	}
}
