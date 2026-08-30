import { describe, expect, it, vi } from 'vitest';
import { BrowserDocumentSession } from './wasm-adapter';

type TestInner = {
	page_count: number;
	inspection: () => unknown;
	render_svg: (pageIndex: number, colorMode: string) => string;
	dispose: () => void;
	free: () => void;
};

function sessionFrom(inner: TestInner): BrowserDocumentSession {
	const Session = BrowserDocumentSession as unknown as new (
		inner: TestInner
	) => BrowserDocumentSession;
	return new Session(inner);
}

describe('BrowserDocumentSession disposal', () => {
	it('disposes and frees the WASM wrapper only once', () => {
		const inner: TestInner = {
			page_count: 1,
			inspection: () => ({}),
			render_svg: () => '<svg/>',
			dispose: vi.fn(),
			free: vi.fn()
		};
		const session = sessionFrom(inner);

		session.dispose();
		session.dispose();

		expect(inner.dispose).toHaveBeenCalledTimes(1);
		expect(inner.free).toHaveBeenCalledTimes(1);
		expect(() => session.summary()).toThrow(/disposed/i);
	});

	it('still frees the wrapper if Rust disposal throws', () => {
		const inner: TestInner = {
			page_count: 1,
			inspection: () => ({}),
			render_svg: () => '<svg/>',
			dispose: vi.fn(() => {
				throw new Error('dispose failed');
			}),
			free: vi.fn()
		};
		const session = sessionFrom(inner);

		expect(() => session.dispose()).toThrow('dispose failed');
		expect(inner.free).toHaveBeenCalledTimes(1);
		expect(() => session.dispose()).not.toThrow();
	});
});
