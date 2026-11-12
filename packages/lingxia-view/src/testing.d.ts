/** A decoded wire node: the element, its test id, own text and the props it sent, by field name. */
export interface WireNode {
  tag: string;
  key: string | null;
  testId: string | null;
  text: string | null;
  props: Record<string, any>;
  children: WireNode[];
  /** Non-enumerable; null at the root. */
  readonly parent: WireNode | null;
}

/** The page's runtime, isolated in its own context. */
export type Host = object;

export interface ViewInput { data?: Record<string, unknown>; env?: Record<string, unknown>; actions?: Record<string, unknown> }

export interface Screen {
  readonly root: WireNode;
  readonly nodes: WireNode[];
  /** The wire buffer of a fresh describe. */
  wire(): unknown[];
  /** Throws on no match, listing the test ids on screen, and on several, naming each. */
  getByTestId(id: string): WireNode;
  /** The innermost node whose text, descendants' included, equals `text`. */
  getByText(text: string): WireNode;
  queryAll(test: (node: WireNode) => boolean): WireNode[];
  /** Fires `onClick`, then re-renders. */
  press(node: WireNode): void;
  /** Fires `onInput` with `text`, then re-renders. */
  type(node: WireNode, text: string): void;
  /** Fires `onCheckedChange`, then re-renders. */
  toggle(node: WireNode, checked: boolean): void;
  /** Merges `data` and `env` into the current view and re-renders; local state is kept. */
  rerender(view: ViewInput): void;
}

/** Builds a GPU page with the View compiler and renders it in Node. */
export function render(page: string, view?: ViewInput, options?: { root?: string }): Promise<Screen>;
export function installHost(source: string): Host;
export function describePage(host: Host, view?: ViewInput): unknown[];
export function dispatch(host: Host, handle: number, value?: unknown): boolean;
export function decode(buffer: unknown[]): WireNode[];
export function find(root: WireNode, test: (node: WireNode) => boolean): WireNode | undefined;
export function textOf(node: WireNode): string;
