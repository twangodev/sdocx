export const nativeStages = [
	'archive', 'resources', 'pages', 'objects', 'layout', 'preparing', 'rendering', 'writingPdf', 'finalizing'
] as const;

export type NativeStage = typeof nativeStages[number];
export type ProgressStage = NativeStage | 'reading' | 'loading' | 'inspecting' | 'saving' | 'hashing' | 'encoding' | 'packaging' | 'ready' | 'displaying';

export interface ProcessingProgress {
	stage: ProgressStage;
	completed: number;
	total: number | null;
}

export type WorkListener = (progress: ProcessingProgress) => void;

const labels: Record<ProgressStage, string> = {
	displaying: 'Displaying preview', reading: 'Reading file', loading: 'Loading browser renderer', archive: 'Reading archive',
	resources: 'Extracting resources', pages: 'Processing pages', objects: 'Decoding objects',
	layout: 'Laying out pages', preparing: 'Preparing text and fonts', rendering: 'Composing vector objects',
	writingPdf: 'Writing PDF pages', finalizing: 'Finalizing output', inspecting: 'Reading document structure',
	saving: 'Saving to library', hashing: 'Checking file identity', encoding: 'Preparing image',
	packaging: 'Packaging download', ready: 'Document ready'
};

export function progressLabel(progress: ProcessingProgress): string {
	return labels[progress.stage];
}

export function pendingProgress(stage: ProgressStage): ProcessingProgress {
	return { stage, completed: 0, total: null };
}

export function readNativeProgress(value: unknown): ProcessingProgress {
	if (typeof value !== 'object' || value === null) throw new Error('Invalid renderer progress.');
	const { stage, completed, total } = value as Record<string, unknown>;
	if (!nativeStages.includes(stage as NativeStage) || !Number.isSafeInteger(completed) || (completed as number) < 0 ||
		(total !== null && (!Number.isSafeInteger(total) || (total as number) < (completed as number)))) {
		throw new Error('Invalid renderer progress.');
	}
	return { stage: stage as NativeStage, completed: completed as number, total: total as number | null };
}
