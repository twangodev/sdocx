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
		emit({ id: request.id, type: 'result', value: await session.handle(request) });
	} catch (error) {
		emit({
			id: request.id,
			type: 'error',
			message: error instanceof Error ? error.message : 'The document could not be processed.'
		});
	}
};

export {};
