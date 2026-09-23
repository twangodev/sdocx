/// <reference lib="webworker" />

import type { ConverterEvent, ConverterRequest } from './protocol';
import { ConverterWorkerSession } from './worker-session';

function emit(event: ConverterEvent): void {
	self.postMessage(event);
}

const session = new ConverterWorkerSession((generation, phase, message) => {
	emit({ type: 'progress', generation, phase, message });
});

self.onmessage = async (event: MessageEvent<ConverterRequest>) => {
	const request = event.data;
	try {
		const value = await session.handle(request);
		self.postMessage(
			{ id: request.id, type: 'result', value } satisfies ConverterEvent,
			value instanceof Uint8Array ? [value.buffer] : []
		);
	} catch (error) {
		emit({
			id: request.id,
			type: 'error',
			message: error instanceof Error ? error.message : 'The document could not be processed.'
		});
	}
};

export {};
