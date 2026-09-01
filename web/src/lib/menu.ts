export interface MenuAction<A> {
	kind: 'action';
	label: string;
	action: A;
	shortcut?: string;
	checked?: boolean;
	disabled?: boolean;
}

export interface MenuSeparator {
	kind: 'separator';
}

export type MenuLeaf<A> = MenuAction<A> | MenuSeparator;

export const separator = (): MenuSeparator => ({ kind: 'separator' });

export const menuItemClass =
	'data-[highlighted]:bg-elevated data-[highlighted]:text-text data-[disabled]:text-muted/45 flex h-7 min-w-52 items-center gap-2 rounded-sm px-2 text-[12px] outline-none data-[disabled]:cursor-default';

export const menuContentClass =
	'motion-menu border-subtle bg-bg z-50 min-w-52 rounded border p-1 shadow-2xl';

export const menuSeparatorClass = 'my-1 h-px bg-subtle';

export const menuCompactItemClass =
	'data-[highlighted]:bg-elevated data-[highlighted]:text-text flex h-7 min-w-32 cursor-default items-center rounded-sm px-2 text-[11px] outline-none';

export const menuCompactContentClass =
	'motion-menu border-subtle bg-bg z-50 min-w-36 rounded border p-1 shadow-2xl';
