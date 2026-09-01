export type ButtonVariant = 'primary' | 'secondary' | 'destructive';

const surfaces: Record<ButtonVariant, string> = {
	primary: 'bg-text text-bg transition-opacity hover:opacity-80',
	secondary: 'border border-subtle text-text transition-colors hover:bg-surface',
	destructive: 'bg-negative text-bg transition-opacity hover:opacity-80'
};

const disabled: Record<ButtonVariant, string> = {
	primary: 'disabled:opacity-35',
	secondary: 'disabled:opacity-40',
	destructive: 'disabled:opacity-45'
};

export function buttonClass(variant: ButtonVariant, options: { busy?: boolean } = {}): string {
	const cursor = options.busy ? 'disabled:cursor-wait' : 'disabled:cursor-not-allowed';
	return `h-7 cursor-pointer rounded px-2 text-[11px] ${surfaces[variant]} ${cursor} ${disabled[variant]}`;
}
