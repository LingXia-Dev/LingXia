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

/** `env` defaults to `defaultEnv()`; the fields given replace its own. */
export interface ViewInput { data?: Record<string, unknown>; env?: Record<string, unknown>; actions?: Record<string, unknown> }

export interface Screen {
  readonly root: WireNode;
  readonly nodes: WireNode[];
  /** The page's runtime, for `dispatch` and the other low-level calls. */
  readonly host: Host;
  /** The wire buffer of a fresh describe. */
  wire(): unknown[];
  /** Throws on no match, listing the test ids on screen, and on several, naming each. */
  getByTestId(id: string): WireNode;
  /** The innermost node whose text, descendants' included, equals `text`. */
  getByText(text: string): WireNode;
  queryAll(test: (node: WireNode) => boolean): WireNode[];
  /** Delivers `value` to the node's handler field (`onClick`, `onSelect`, …), then re-renders. */
  fire(node: WireNode, field: string, value?: unknown): void;
  /** `onClick`. */
  press(node: WireNode): void;
  /** `onInput`. */
  type(node: WireNode, text: string): void;
  /** `onSubmit`, by default with the editor's current value. */
  submit(node: WireNode, text?: string): void;
  /**
   * Flips a checkbox, switch or radio option. With `checked`, throws when it is already in that
   * state, or when it is not in it afterwards (a controlled `onChange` that did not update `value`).
   */
  toggle(node: WireNode, checked?: boolean): void;
  /** `<slider>` `onChange`; the value snaps to the step. */
  slide(node: WireNode, value: number): void;
  /** `<select>` `onChange` with an option's value. */
  select(node: WireNode, value: string): void;
  /** `<plot>` `onSelect`; `-1` clears. */
  selectPoint(node: WireNode, index: number): void;
  /** A `<scroll>`/`<List>`'s `onReachStart`/`onReachEnd`. */
  reach(node: WireNode, edge: "start" | "end"): void;
  /** `<image>` `onLoad`/`onError`. */
  image(node: WireNode, outcome: "load" | "error"): void;
  /** A snapping `<scroll>` coming to rest on snap point `index`. */
  snap(node: WireNode, index: number): void;
  /** Lays the node out at this content size: its `onLayout` and `useBox` handle take it. */
  layout(node: WireNode, box: { width: number; height: number }): void;
  /** A `drag` element's gesture: `start`, `end` (`index`, `velocity`) or `settle` (`index`, `hidden`). */
  drag(node: WireNode, event: { phase: "start" | "end" | "settle"; index?: number; velocity?: number; hidden?: boolean }): void;
  /** `<video>` events (`{ type: "play" }`, …) with the player's status. */
  video(node: WireNode, status: object, events?: ReadonlyArray<{ type: string; [detail: string]: unknown }>): void;
  /** The Back or Escape a device sends: closes the topmost overlay; false when none was open. */
  escape(): boolean;
  /** Merges `data` and `env` into the current view and re-renders; local state is kept. */
  rerender(view: ViewInput): void;
}

/** A phone: 390×844 light, ltr, `en-US`, coarse pointer, no insets; `overrides` replace fields. */
export function defaultEnv(overrides?: Record<string, unknown>): Record<string, unknown>;

/** Builds a GPU page with the View compiler and renders it in Node. */
export function render(page: string, view?: ViewInput, options?: { root?: string }): Promise<Screen>;
export function installHost(source: string): Host;
export function describePage(host: Host, view?: ViewInput): unknown[];
export function dispatch(host: Host, handle: number, value?: unknown): boolean;
export function decode(buffer: unknown[]): WireNode[];
export function find(root: WireNode, test: (node: WireNode) => boolean): WireNode | undefined;
export function textOf(node: WireNode): string;
