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

/** Logic's end of a page channel, as a `channels` handler gets it. */
export interface ServedChannel {
  /** Delivers `payload` to the page's `onData`. */
  send(payload: unknown): void;
  /** Closes the channel from Logic's side; pass `error` when it ends in failure. */
  close(error?: { code: string; message: string; reason?: string; data?: unknown }): void;
  /** `data`: what the page's `send` sent; `close`: the page closed it (`error` when in failure). Returns an unsubscribe. */
  on(event: "data", listener: (payload: unknown) => void): () => void;
  on(event: "close", listener: (closed: { error?: { code: string; message: string; reason?: string; data?: unknown } }) => void): () => void;
}

/**
 * `env` defaults to `defaultEnv()`; the fields given replace its own. `channels` plays Logic's side of
 * the page's channels: each name becomes a page action a `useChannel` opens through the page's own
 * bridge, and its handler is called once per open with Logic's end and the params.
 */
export interface ViewInput {
  data?: Record<string, unknown>; env?: Record<string, unknown>; actions?: Record<string, unknown>;
  channels?: Record<string, (channel: ServedChannel, params: unknown) => void>;
  /** The clock `useNow` and event `timeStamp`s read, epoch ms; it moves only by `advance`. Defaults to when `render` ran. */
  now?: number;
}

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
  /** A `<scroll>`/`<List>`'s `onVisibleChange` with these children on screen, in order: an element by `id`, a `List` row by `index`. */
  visible(node: WireNode, items: ReadonlyArray<{ id?: string; index?: number; ratio: number; top: number }>): void;
  /** `<image>` `onLoad`/`onError`. */
  image(node: WireNode, outcome: "load" | "error"): void;
  /** A snapping `<scroll>` coming to rest on snap point `index`. */
  snap(node: WireNode, index: number): void;
  /** A key the editor's `onKeyDown` map lists, as the host delivers it. */
  key(node: WireNode, key: string): void;
  /** The person moves the caret to `start`, or selects `start..end`, in an editor: its `onSelectionChange`. */
  selectText(node: WireNode, start: number, end?: number): void;
  /** A paste into an editor: `onPaste` hears it first, then, unless it called `preventDefault()`, the plain text goes in at `selection` (default: the end) as `onInput`. */
  paste(node: WireNode, clipboard: { text: string; html?: string; plain?: boolean; selection?: { start: number; end: number } }): { prevented: boolean };
  /** `onFocus` on an editor, button or link. An editor's `focusVisible` is always true; a control's is `visible` (a key moved it), false by default. */
  focus(node: WireNode, options?: { visible?: boolean }): void;
  /** `onBlur`, with `focusVisible` as `focus` reports it. */
  blur(node: WireNode, options?: { visible?: boolean }): void;
  /** An `onPointer` event at a point of the node's content box; `pointer` defaults to `touch`. */
  pointer(node: WireNode, event: { phase: "down" | "move" | "up" | "cancel" | "hover" | "leave" | "increment" | "decrement"; x: number; y: number; width: number; height: number; pointer?: "touch" | "mouse" | "pen" | "keyboard" }): void;
  /** Lays the node out at this content size, with its content's min- and max-content widths when given: its `onLayout` and `useBox` handle take it. */
  layout(node: WireNode, box: { width: number; height: number; minContentWidth?: number; maxContentWidth?: number }): void;
  /** A `drag` element's gesture: `start`, `end` (`index`, `velocity`) or `settle` (`index`, `hidden`). */
  drag(node: WireNode, event: { phase: "start" | "end" | "settle"; index?: number; velocity?: number; hidden?: boolean }): void;
  /** `<video>` events (`{ type: "play" }`, …) with the player's status. */
  video(node: WireNode, status: object, events?: ReadonlyArray<{ type: string; [detail: string]: unknown }>): void;
  /** `<webview>` events (`{ type: "load", url, title }`, `{ type: "message", json, origin }`, …). */
  webview(node: WireNode, events?: ReadonlyArray<{ type: string; [detail: string]: unknown }>): void;
  /** The Back or Escape a device sends: closes the topmost overlay; false when none was open. */
  escape(): boolean;
  /** Moves the clock on by `ms`; a page showing `useNow` re-renders when its step changes. */
  advance(ms: number): void;
  /** The test's clock, epoch ms. */
  readonly now: number;
  /** Merges `data` and `env` into the current view and re-renders; local state is kept. */
  rerender(view: ViewInput): void;
  /** Re-renders until the page's channel frames and the promises they settle have run out. */
  settle(): Promise<void>;
  /** With `channels`, the other messages the page sent (`nav.go`, …), unanswered, oldest first. */
  takeRequests(): Array<{ kind: string; [field: string]: unknown }>;
}

/** A phone: 390×844 light, ltr, `en-US`, coarse pointer, no insets; `overrides` replace fields. */
export function defaultEnv(overrides?: Record<string, unknown>): Record<string, unknown>;

/**
 * Builds a GPU page with the View compiler and renders it in Node. With `points`, every node's props
 * read in points under the built-in pack (see `points`), across re-renders. `messages` is what `t`
 * reads: a compiled i18n `index.js`, or `.lingxia/i18n/build` after `lingxia build`.
 */
export function render(page: string, view?: ViewInput, options?: { root?: string; points?: boolean; messages?: string | null }): Promise<Screen>;
export function installHost(source: string): Host;
export function describePage(host: Host, view?: ViewInput): unknown[];
export function dispatch(host: Host, handle: number, value?: unknown): boolean;
export function decode(buffer: unknown[]): WireNode[];
export function find(root: WireNode, test: (node: WireNode) => boolean): WireNode | undefined;
export function textOf(node: WireNode): string;
/**
 * `value` with every theme step (`[slot, times, plus]`: `p-4`, `text-sm`, `rounded-lg`, `duration-fast`)
 * resolved as a host does under the built-in pack, at any depth; `em` steps stay as they are, and a
 * list on a field that takes no step is left alone.
 */
export function points<T>(value: T): T;
