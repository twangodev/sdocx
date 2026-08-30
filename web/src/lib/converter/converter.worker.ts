/// <reference lib="webworker" />

import type { ConverterEvent, ConverterRequest, WorkerPhase } from './protocol';
import { BrowserDocumentSession } from './wasm-adapter';

let session: BrowserDocumentSession | undefined;

function emit(event: ConverterEvent): void {
	self.postMessage(event);
}

function progress(phase: WorkerPhase, message: string): void {
	emit({ type: 'progress', phase, message });
}

async function handle(request: ConverterRequest): Promise<unknown> {
	switch (request.type) {
		case 'load': {
			progress('loading', 'Loading the browser renderer');
			const previous = session;
			session = undefined;
			previous?.dispose();
			progress('parsing', 'Parsing document locally');
			let next: BrowserDocumentSession | undefined;
			try {
				next = await BrowserDocumentSession.create(request.bytes);
				progress('inspecting', 'Reading document structure');
				const summary = next.summary();
				session = next;
				progress('ready', 'Document ready');
				return summary;
			} catch (error) {
				try {
					next?.dispose();
				} catch {
					// Preserve the parse or inspection error instead of masking it with cleanup.
				}
				throw error;
			}
		}
		case 'inspect':
			return requireSession().inspection();
		case 'renderPage':
			progress('rendering', `Rendering page ${request.pageIndex + 1}`);
			return requireSession().renderPage(request.pageIndex, request.colorMode);
		case 'exportJson':
			return JSON.stringify(requireSession().inspection(), null, 2);
		case 'dispose':
			session?.dispose();
			session = undefined;
			return null;
	}
}

function requireSession(): BrowserDocumentSession {
	if (!session) throw new Error('Load a document before requesting its contents.');
	return session;
}

self.onmessage = async (event: MessageEvent<ConverterRequest>) => {
	const request = event.data;
	try {
		emit({ id: request.id, type: 'result', value: await handle(request) });
	} catch (error) {
		emit({
			id: request.id,
			type: 'error',
			message: error instanceof Error ? error.message : 'The document could not be processed.'
		});
	}
};

export {};
