/** Per-layout state shared by the app bar and its loaded document. */
export interface WorkspaceState {
	hasDocument: boolean;
	debuggerOpen: boolean;
}
export const WORKSPACE = Symbol('document workspace');
