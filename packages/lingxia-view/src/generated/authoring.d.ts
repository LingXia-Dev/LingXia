// Generated from contracts/view/schema.json; do not edit.

export type Child = JSX.Element | string | number | boolean | null | undefined | readonly Child[];
export type TextChild = string | number | boolean | null | undefined | readonly TextChild[];
/** Text content of `<text>` and `<span>`: strings, `<span>` and `<link>` runs. */
export type RunChild = TextChild | JSX.Element | readonly RunChild[];
export type ComponentResult = JSX.Element | null | undefined | boolean;
export type Component<P = {}> = (props: P) => ComponentResult;
export type ComponentProps<C> = C extends (props: infer P) => unknown ? P : never;
/** A day of the week, 1 = Monday … 7 = Sunday, as Temporal and ISO 8601; `Date.getDay()` counts 0 = Sunday. */
export type Weekday = 1 | 2 | 3 | 4 | 5 | 6 | 7;
/** Groups children without a box: `<>…</>`, or `<Fragment key>` to move a group together in a keyed list. */
export declare function Fragment(props: { key?: string; children?: Child }): JSX.Element;
export declare function childrenToArray(children: Child): readonly Exclude<Child, readonly Child[] | boolean | null | undefined>[];
/** What `child` is: an intrinsic tag name, the component it renders, `Fragment`, or undefined for text, null, booleans and arrays. Compare a component by identity: `elementType(c) === Button`. */
export declare function elementType(child: Child): string | Component<any> | undefined;
/** A copy of `element` with `props` merged over its own, in the original's place: its key, children and local state stay. `className` is joined, not replaced. A handler (`on*`) the element already has is replaced only when named in `override`; otherwise it throws `UI_PROP`. Anything but one element (text, null, an array) throws `UI_PROP`. */
export declare function cloneElement<P = Record<string, unknown>>(
  element: Child,
  props: Partial<P> & { key?: never; children?: never },
  options?: { override?: readonly (keyof P & `on${string}`)[] },
): JSX.Element;

/** Props every element accepts; `style` is narrowed per element. */
/** `dir` sets the inline direction of a subtree; otherwise it follows `env.direction`. Editors also take `"auto"` (first strong character). */
export interface Props {
  style?: Style; className?: string; key?: string; "data-testid"?: string; dir?: "ltr" | "rtl" | "auto"; children?: Child;
  /** Unique on the page (`UI_ID`); what `popoverTarget` and `aria-actions` name. Build one with `useId()`. */
  id?: string;
  /** Semantic role; controls keep their own. Headings take `headingLevel` 1..6; `alertdialog` goes on a `<dialog>` that interrupts.
   * Composites move focus among their items with the arrow keys and take one Tab stop: `tablist` › `tab`, `radiogroup` › `radio`,
   * `toolbar` › any control, `listbox` › `option`, `menu` › `menuitem`/`menuitemcheckbox`/`menuitemradio`, `tree` › `treeitem`.
   * Item roles go on a `<button>`. `img` on a `<view>` or `<text>` with an `aria-label` speaks that label in place of its content.
   * `dialog` goes on a popover `<view>` (with `aria-modal` for a modal one); a `<dialog>` has it already. */
  role?: "heading" | "list" | "listitem" | "dialog" | "alert" | "status" | "grid" | "gridcell" | "listbox" | "option" | "menu" | "menuitem" | "progressbar" | "alertdialog" | "group" | "tooltip"
    | "tablist" | "tab" | "tabpanel" | "radiogroup" | "radio" | "toolbar" | "tree" | "treeitem" | "row" | "menuitemcheckbox" | "menuitemradio" | "img" | "combobox" | "spinbutton";
  /** Removes the node and its subtree from accessibility. */
  "aria-hidden"?: boolean;
  /** Announce changes; `alert` is assertive and `status` polite unless set. */
  "aria-live"?: "off" | "polite" | "assertive";
  /** States assistive input announces; leave one out when the element has no such state. */
  "aria-pressed"?: boolean; "aria-selected"?: boolean; "aria-expanded"?: boolean; "aria-busy"?: boolean;
  /** A field's error and required states; the error text belongs in `aria-describedby`. */
  "aria-invalid"?: boolean; "aria-required"?: boolean;
  /** Ids of elements (1..8, space-separated) whose text is read after the name, as a hint or error. */
  "aria-describedby"?: string;
}
export type LeafProps = Omit<Props, "children"> & { children?: never };


export type ReadonlyData<T> = T extends object ? { readonly [K in keyof T]: ReadonlyData<T[K]> } : T;
/** A failed resource or action, as reported by the bridge. */
/** `code` is the `E_` string and `reason` names the business case (camelCase); `message` is English for a log. */
export interface ResourceError {
  readonly code: string; readonly message: string;
  readonly reason?: string; readonly data?: unknown;
}

export interface Stream<T = unknown, R = unknown> extends AsyncIterable<ReadonlyData<T>> {
  readonly id: string; readonly result: Promise<ReadonlyData<R>>; cancel(): void;
  on(kind: "data", listener: (value: ReadonlyData<T>) => void): this;
  on(kind: "end", listener: (value: ReadonlyData<R>) => void): this;
  on(kind: "error", listener: (error: Error & { code?: string }) => void): this;
}
export interface Channel<T = unknown, S = unknown> extends AsyncIterable<ReadonlyData<T>> {
  readonly id: string; send(payload: S): void; close(code?: string, reason?: string): void;
  on(kind: "data", listener: (value: ReadonlyData<T>) => void): this;
  on(kind: "close", listener: (code?: string, reason?: string) => void): this;
  on(kind: "error", listener: (error: Error & { code?: string }) => void): this;
}
export interface Channels { open<T = unknown, S = unknown>(topic: string, params?: unknown): Promise<Channel<T, S>>; }

/** The Logic side of a channel: sends `T` to the View and receives `S`. */
export interface ChannelEndpoint<T = unknown, S = unknown> {
  send(payload: T): void;
  close(code?: string, reason?: string): void;
  on(event: "data", handler: (payload: S) => void): () => void;
  on(event: "close", handler: (info: { code: string; reason: string }) => void): () => void;
}

/** A page's actions as the View calls them: a Promise of the result, or a `Stream` for an `async *` action. */
export type ViewActions<A> = { readonly [K in keyof A]:
  A[K] extends (...args: infer P) => AsyncGenerator<infer T, infer R, any>
    ? (P extends [] | [unknown?] ? (...args: P) => Stream<T, R> : never)
  : A[K] extends (...args: infer P) => Promise<infer R>
    ? (P extends [] | [unknown?] ? (...args: P) => Promise<ReadonlyData<R>>
      : P extends [any, ChannelEndpoint<any, any>] ? A[K] : never)
  : A[K] extends (...args: infer P) => unknown
    ? (P extends [] | [unknown?] | [any, ChannelEndpoint<any, any>] ? A[K] : never)
  : A[K] };
export interface PageView<A = Readonly<Record<string, (payload?: unknown) => Promise<unknown>>>, D = Record<string, unknown>> {
  readonly data: ReadonlyData<D>; readonly actions: ViewActions<A>; readonly channels: Channels; readonly env: Env;
}
/**
 * The View of a page. A page takes it typed from its Logic, `import type { View } from './$types'`;
 * `View<typeof page>` and `View<Actions, Data>` are the same type spelled out.
 */
export type View<A = Readonly<Record<string, (payload?: unknown) => Promise<unknown>>>, D = Record<string, unknown>> =
  A extends { readonly __lxPage: unknown; readonly data: infer PD; readonly actions: infer PA } ? PageView<PA, PD> : PageView<A, D>;
/** The View of the page being rendered; components needing typed data take it as props. */
/** The page's environment alone: a component that reads only this re-runs when a field it read changes, not when `view.data` does. */
export declare function useEnv(): Env;
export declare function useView<A = Readonly<Record<string, (payload?: unknown) => Promise<unknown>>>, D = Record<string, unknown>>(): View<A, D>;

declare const localCell: unique symbol;
/**
 * JSON-shaped state; undefined object fields are omitted, including optional fields. An `unknown`
 * value (an open record's `Record<string, unknown>`) is taken as is and checked when it is set.
 */
export type LocalData<T> = unknown extends T ? T : T extends string | number | boolean | null ? T
  : T extends (...args: any[]) => any ? never
  : T extends readonly unknown[] ? { [K in keyof T]: LocalData<T[K]> }
  : T extends object ? { [K in keyof T]: T[K] extends undefined ? undefined : LocalData<T[K]> } : never;
/** A cell of component-local state: `useLocal`'s result, and what an element's `bind` takes. */
export interface Local<T> {
  readonly [localCell]: true; readonly value: ReadonlyData<T>;
  /** Sets the value. */
  set(value: ReadonlyData<LocalData<T>>): void;
  /** Derives the value from the latest one: `count.set(n => n + 1)`. */
  set(update: (latest: ReadonlyData<T>) => ReadonlyData<LocalData<T>>): void;
}
/** A text cell (`useLocal('')`): a `Local` an editor can bind, so it can focus that editor. */
export interface LocalText<T extends string = string> extends Local<T> {
  /** Focus the editor this cell is bound to (`bind`), on the next frame. */
  focus(): void;
  /**
   * Open the picker of the date or time editor this cell is bound to, as a tap does, on the next
   * frame; nothing while it is disabled or read-only. The picker closes itself.
   */
  showPicker(): void;
  /**
   * Select `start..end` (UTF-16 offsets, `start <= end`; equal for a caret) in the editor this cell
   * is bound to. A focused editor shows it on the next frame; an unfocused one keeps it for its next focus.
   */
  setSelection(start: number, end: number): void;
  /**
   * Replace `start..end` of the value with `text` and leave the caret after it, as typing over a
   * selection would; the value changes at once, the editor's caret on the next frame.
   */
  replaceRange(start: number, end: number, text: string): void;
}
/** Local state keyed by call order in the component, as React's `useState`. */
export declare function useLocal<T>(initial: T, ...invalid: [T] extends [LocalData<T>] ? [] : [never]): [T] extends [string] ? LocalText<T & string> : Local<T>;
/** Local state keyed by `name`: it survives reordered calls and hot edits. */
export declare function useLocal<T>(name: string, initial: T, ...invalid: [T] extends [LocalData<T>] ? [] : [never]): [T] extends [string] ? LocalText<T & string> : Local<T>;

/** A value a `Provider` hands its subtree: plain data, functions or handles such as `Local`. */
export interface Context<T> {
  /** Wraps exactly one element; a changed `value` re-runs only the components that read it. */
  readonly Provider: (props: { value: T; key?: string; children?: JSX.Element | null | false }) => JSX.Element;
  readonly defaultValue: T;
}
/** Declare as a module-level `const`; the value with no `Provider` above is `defaultValue`. */
export declare function createContext<T>(defaultValue: T): Context<T>;
/** The nearest enclosing `Provider`'s value, during render only. */
export declare function useContext<T>(context: Context<T>): T;
/** An element `id` unique to this component instance (and list row), stable across renders and hot swaps. */
export declare function useId(): string;
/**
 * The lxapp's message `key` from `i18n/<tag>.json` in the page's language, with ICU `args`
 * (`{ count: 3 }`); a switch of language re-renders the page. During render only.
 */
export declare function t(...message: MessageCall): string;
/** A component library's words, as `lingxia gen i18n` compiles them: `(args, formatter) => text` per key. */
export interface PackageWords {
  readonly source: string;
  readonly languages: Readonly<Record<string, Readonly<Record<string, (args: any, formatter: any) => unknown>>>>;
  formatter(tag: string): unknown;
}
/**
 * A component library's word `key` (`@liuli.dialog.close`) in the page's language: the lxapp's own
 * catalogue entry for the key when it has one, else the package's, else the package's source
 * language. During render only.
 */
export declare function packageText(words: PackageWords, key: string, args?: Readonly<Record<string, unknown>>): string;

declare global {
  /**
   * The lxapp's messages, each with the arguments it takes. Empty here: the build generates the
   * source catalogue's keys into it, and until then any key compiles.
   */
  interface LxAppMessages {}
}

declare const messageKey: unique symbol;
/** A key built at run time, named `as MessageKey`: still one of the lxapp's keys, its arguments unchecked. */
export type MessageKey = (keyof LxAppMessages extends never ? string : keyof LxAppMessages & string) & { readonly [messageKey]: true };

/** `t`'s key and arguments, checked against `LxAppMessages` once the build generates it. */
export type MessageCall = keyof LxAppMessages extends never
  ? [key: string, args?: Readonly<Record<string, unknown>>]
  : {
      [K in keyof LxAppMessages & string]: {} extends LxAppMessages[K]
        ? [key: K, args?: LxAppMessages[K]]
        : [key: K, args: LxAppMessages[K]];
    }[keyof LxAppMessages & string]
    | [key: MessageKey, args?: Readonly<Record<string, unknown>>];

export interface RequestResource<T> {
  /** A load is in flight; `value` keeps the previous result until it settles. */
  readonly pending: boolean;
  /** `value` belongs to an earlier load than the one in flight or failed. */
  readonly stale: boolean;
  readonly value: ReadonlyData<T> | null;
  readonly error: ResourceError | null;
  reload(): Promise<void>;
}
export declare function useRequest<P extends [] | [unknown?], R>(name: string, action: (...params: P) => Promise<R>, ...params: P): RequestResource<R>;

export interface ActionResource<P extends unknown[], R> {
  /** The latest `run` is in flight. */
  readonly pending: boolean;
  /** The latest `run`'s failure, or `null`; cleared when the next one starts. */
  readonly error: ResourceError | null;
  /** Call the action. Never rejects: resolves `{ ok: true, value }`, or `{ ok: false, error }` as `error` also shows. Usable as a handler. */
  run(...params: P): Promise<ActionOutcome<R>>;
}
/** How one `run` settled; an action that returns nothing succeeds with `value` `null`, as JSON carries it. */
export type ActionOutcome<R> = { readonly ok: true; readonly value: [R] extends [void] ? null : ReadonlyData<R> } | { readonly ok: false; readonly error: ResourceError };
/** One action called from handlers, with its busy and failed state; keyed by call order in the component. */
export declare function useAction<P extends [] | [unknown?], R>(action: (...params: P) => Promise<R>): ActionResource<P, R>;
/** `useAction` keyed by `name`, for a second use of one action in a component. */
export declare function useAction<P extends [] | [unknown?], R>(name: string, action: (...params: P) => Promise<R>): ActionResource<P, R>;

export interface StreamResource<P extends unknown[], R, T = unknown> {
  readonly status: "idle" | "streaming" | "completed" | "canceled" | "failed";
  readonly error: ResourceError | null;
  readonly result: ReadonlyData<R> | null;
  /** The latest value the stream yielded; `null` before the first and after a restart. */
  readonly data: ReadonlyData<T> | null;
  start(...params: P): void;
  cancel(): void;
}
export declare function useStream<P extends [] | [unknown?], T, R>(name: string, action: (...params: P) => Stream<T, R>, options?: { onData?: (value: ReadonlyData<T>) => void }): StreamResource<P, R, T>;

export interface ChannelResource<S = unknown> {
  readonly connecting: boolean; readonly connected: boolean; readonly error: ResourceError | null;
  connect(): Promise<void>; close(): void; send(payload: S): void;
}
export declare function useChannel<P, T, S>(
  name: string,
  action: (params: P, channel: ChannelEndpoint<T, S>) => unknown,
  options: { params: P; onData?: (message: ReadonlyData<T>) => void },
): ChannelResource<S>;

export type VideoErrorCode = "VIDEO_SOURCE" | "VIDEO_NOT_GRANTED" | "VIDEO_NETWORK" | "VIDEO_NOT_FOUND" | "VIDEO_FORMAT" | "VIDEO_DECODE" | "VIDEO_DECODERS" | "VIDEO_NO_PLAYER" | "VIDEO_UNSUPPORTED";
/** Says what to do in `message`; `recoverable` errors resume with `retry()`. */
/** When the host took the event, in epoch ms (the base of `Date.now()`). */
export interface TimeStamped { readonly timeStamp: number }
/** A press of a button or link. */
export type PressEvent = TimeStamped;
export interface VideoError { readonly code: VideoErrorCode; readonly message: string; readonly recoverable: boolean }
export type WebViewErrorCode = "WEBVIEW_SOURCE" | "WEBVIEW_NOT_GRANTED" | "WEBVIEW_NETWORK" | "WEBVIEW_CRASHED" | "WEBVIEW_UNSUPPORTED" | "WEBVIEW_MESSAGE_DROPPED";
export interface WebViewError { readonly code: WebViewErrorCode; readonly message: string }
/** A top-level document finished loading; an in-page navigation reports too. */
export interface WebViewLoad { readonly url: string; readonly title: string }
/** `data` is what the page passed to `window.lingxia.postMessage`; the host fills `origin` ("lxapp" for a packaged or `lx://` file). */
export interface WebViewMessage { readonly data: unknown; readonly origin: string }
/** A navigation the host cancelled: an address outside trustedDomains, or a scheme other than https. */
export interface WebViewBlocked { readonly url: string }
/** A page's embedded web document: pass it as `web` to each `<webview>` that shows it (for example an inline card and an open `<dialog>` or sheet), so moving between them keeps one live document without reloading. */
export interface WebView {
  readonly url: string;
  readonly title: string;
  readonly error: WebViewError | null;
  /** Shown full screen: the page draws it in an open `<dialog>` holding a `<webview>` of this handle; without one, the bare page fills the window. */
  readonly fullscreen: boolean;
  /** One `<webview>` at a time; Back or `exitFullscreen()` leaves. */
  requestFullscreen(): Promise<void>;
  exitFullscreen(): Promise<void>;
}
export declare function useWebView(name: string): WebView;
export interface VideoMetadata { readonly duration: number | null; readonly videoWidth: number; readonly videoHeight: number; readonly live: boolean }
/** One quality of a `sources` video: `label` is what `setQuality` and `quality` name. */
export interface VideoSource { readonly src: string; readonly label: string }
/** A `<video>`'s source as a forwarding component takes it: spread it whole onto the `<video>`. */
export type VideoSourceProps = { src: string; sources?: never } | { sources: readonly VideoSource[]; src?: never } | { src?: never; sources?: never };
/** `quality` is the label now on screen; `auto` while the player picks it. */
export interface VideoQualityChange { readonly quality: string; readonly auto: boolean }
export interface VideoPause { readonly reason: "user" | "api" | "hidden" | "interrupted" | "replaced" | "parked" }
/** A page's player: pass it as `player` to each `<video>` that shows it. Reading a field re-renders on its change. */
export interface Video {
  readonly paused: boolean; readonly ended: boolean; readonly buffering: boolean; readonly seeking: boolean;
  readonly readyState: "empty" | "metadata" | "ready";
  /** Seconds; `duration` is null before metadata and for live streams. `buffered` is seconds ready ahead. */
  readonly currentTime: number; readonly duration: number | null; readonly buffered: number;
  readonly videoWidth: number; readonly videoHeight: number;
  readonly playbackRate: number; readonly volume: number; readonly muted: boolean;
  /** `"auto"` or the label chosen with `setQuality`; `qualities` are the labels to choose from. `"auto"` adapts but never past the smallest rendition that fills the picture (on HarmonyOS the system player chooses); a chosen label is not capped. */
  readonly quality: string; readonly qualities: readonly string[];
  readonly error: VideoError | null;
  /** Shown full screen: the page draws it in an open dialog holding a `<video>` of this player; without one, the bare picture fills the window. */
  readonly fullscreen: boolean;
  play(): Promise<void>; pause(): Promise<void>; seek(seconds: number): Promise<void>;
  setMuted(muted: boolean): Promise<void>; setVolume(volume: number): Promise<void>; setPlaybackRate(rate: number): Promise<void>;
  /** Switches at a keyframe without a restart; resolves once asked, `onQualityChange` fires when on screen. */
  setQuality(quality: string): Promise<void>;
  retry(): Promise<void>;
  /** One player at a time; back or `exitFullscreen()` leaves. */
  requestFullscreen(): Promise<void>; exitFullscreen(): Promise<void>;
}
export declare function useVideo(name: string): Video;

/**
 * Where a pointer is over an `onPointer` element: points from the top-left of its content box
 * (`width` x `height`), not clamped to it and not mirrored in a right-to-left layout. `hover` and
 * `leave` come from a mouse with no button down; `cancel` when a scroll takes the gesture. An element
 * that also gives `aria-valuenow` is a keyboard stop: arrows on its axis and a screen reader's
 * adjust send `increment` or `decrement` with `pointer: "keyboard"` and no position.
 */
/** A request for an element's menu: a long press, a secondary click, or the Menu key. */
/** Where a `<link to>` goes: a target of `lx.nav.go`, or `{ back: n }`. Logic navigates. */
export type LinkTarget =
  | LinkPageTarget
  | { readonly page: LinkOpenPageName; readonly params?: never; readonly replace?: true }
  | { readonly tab: LinkTabName; readonly replace?: never }
  | { readonly app: string; readonly page?: string; readonly params?: Readonly<Record<string, unknown>>; readonly channel?: "release" | "draft"; readonly version?: string; readonly replace?: never }
  | { readonly url: string; readonly replace?: never }
  | { readonly back: number; readonly replace?: never };

/** A page of this lxapp that is not a tab root; `params` is required when the page requires one. `replace` swaps the current page instead of pushing. */
export type LinkPageTarget<N extends LinkPageName = LinkPageName> = N extends unknown
  ? { readonly page: N; readonly replace?: true } & ({} extends LinkParams<N> ? { readonly params?: LinkParams<N> } : { readonly params: LinkParams<N> })
  : never;
/** Pages that take no required params: a target naming one of several (a union) checks without params. */
export type LinkOpenPageName<N extends LinkPageName = LinkPageName> = N extends unknown ? ({} extends LinkParams<N> ? N : never) : never;
/** This lxapp's page names, tab roots excepted; any string before the project's registry is generated. */
export type LinkPageName = keyof LxAppPages extends never ? string : Exclude<keyof LxAppPages, keyof LxAppTabs> & string;
export type LinkTabName = keyof LxAppPages extends never ? string : keyof LxAppTabs & string;
export type LinkParams<N> = N extends keyof LxAppPageTypes
  ? LxAppPageTypes[N] extends { readonly __lxPage: { readonly params: infer T } } ? Readonly<T> : Readonly<Record<string, unknown>>
  : Readonly<Record<string, unknown>>;

declare global {
  /** The project's page registry, generated into `.lingxia/types/`; empty until then. */
  interface LxAppPages {}
  interface LxAppTabs {}
  interface LxAppPageTypes {}
}

export interface ContextMenuEvent {
  /** The point in page coordinates; the element's centre from the keyboard. */
  readonly x: number;
  readonly y: number;
  readonly pointer: "touch" | "mouse" | "keyboard";
}

export interface PointerEvent {
  readonly phase: "down" | "move" | "up" | "cancel" | "hover" | "leave" | "increment" | "decrement";
  readonly x: number; readonly y: number; readonly width: number; readonly height: number;
  readonly pointer: "touch" | "mouse" | "pen" | "keyboard";
}
/**
 * Keys an editor hands to the page while it keeps the caret. Only the keys listed go to the page,
 * and a listed key does nothing else (no caret move, no newline, no submit); a held key repeats.
 * An input method that is composing keeps every key. `Escape` left out closes the topmost overlay.
 */
export type EditorKeys = Partial<Record<"ArrowUp" | "ArrowDown" | "Enter" | "Escape" | "Home" | "End" | "PageUp" | "PageDown", () => void>>;
/**
 * An element's laid-out content box, inside its padding and border, in points, and the widths
 * its content would take at its narrowest (`minContentWidth`: every line broken, as CSS
 * `min-content`) and with no line broken (`maxContentWidth`, as `max-content`).
 */
export interface BoxSize {
  readonly width: number; readonly height: number;
  readonly minContentWidth: number; readonly maxContentWidth: number;
}
/** A box in page points. */
export interface TextRect { readonly x: number; readonly y: number; readonly width: number; readonly height: number }
/**
 * An editor's selection: UTF-16 offsets into its value (`start <= end`; equal for a caret), and where
 * it shows, in page points: `caret` (null while scrolled out of view) and a rect per selected line.
 */
export interface EditorSelection {
  readonly start: number; readonly end: number;
  readonly caret: TextRect | null; readonly rects: readonly TextRect[];
}
/** A scroll or list reached (`at: true`) or left an edge. Fires once per change, and on mount with the edges its content starts at: both when the content fits. */
export interface ScrollEdgeEvent { readonly edge: "start" | "end"; readonly at: boolean }
/** A child on screen: `top` is its start edge from the viewport's, `ratio` the share of it shown. */
/** A paste into an editor, before anything is inserted. `files` and `omitted` list what a later version copies in or leaves out; today `files` is always empty. */
export interface PasteEvent extends TimeStamped {
  readonly text: string;
  readonly html?: string;
  readonly files: readonly PastedFile[];
  readonly omitted: readonly ("html" | "files")[];
  /** The UTF-16 range the paste replaces. */
  readonly selection: { readonly start: number; readonly end: number };
  /** "Paste as plain text" or "Paste and Match Style": no `html`. */
  readonly plain: boolean;
  readonly defaultPrevented: boolean;
  /** Called synchronously: nothing is inserted, the handler inserts its own result. */
  preventDefault(): void;
}
export interface PastedFile { readonly path: string; readonly type: string; readonly name?: string; readonly size: number }
export interface VisibleItem { readonly id: string; readonly index?: number; readonly ratio: number; readonly top: number }
/** The children on screen in order; `current` is the last that reached the top, else the first. */
export interface VisibleChangeEvent extends TimeStamped { readonly items: readonly VisibleItem[]; readonly current?: string }
/** The size of the element that takes this handle as `box`; `null` until its first layout, then one render behind a change. */
export interface Box { readonly value: BoxSize | null }
export declare function useBox(name: string): Box;
/** The wall clock in epoch ms, floored to the `second` or `minute`. The page re-renders when it steps, only while shown; a test's pinned `now` holds it. */
export declare function useNow(granularity: "second" | "minute"): number;
/**
 * From a handler: scrolls the element with this `id` into view inside its `<scroll>`s, once the
 * render the handler causes is on screen. A `<List>` row that is not rendered has no element; use
 * the list's `anchor`.
 */
export declare function reveal(id: string): void;

export type LeaveReason = "back" | "home";
export interface LeaveGuard {
  readonly pending: LeaveReason | null; readonly requests: number; readonly error: ResourceError | null;
  answer(leave: boolean): void; request(reason: LeaveReason): void;
}
export declare function useLeaveGuard(name: string, options: { enabled: boolean }): LeaveGuard;

/** Logical edge insets (in logical dp/pt). `inlineStart`/`inlineEnd` mirror `left`/`right` in LTR and flip in RTL. */
export interface Edges {
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
  readonly left: number;
  readonly inlineStart: number;
  readonly inlineEnd: number;
}

/** Physical display hinge/crease intersecting the page canvas, in page-local logical coordinates. */
export interface HingeRect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly orientation: "vertical" | "horizontal";
  readonly posture: "flat" | "half-opened";
}

/** Edges on which a direct child of `<page>` extends to the physical screen edge beneath bars and cutouts. */
export type BleedEdge = boolean | "top" | "bottom" | "horizontal";

/** Overrides `env.sizeClass` and `regular:` class matching for its subtree (used by `<ListDetail>` / `<Split>`). */
export declare function SizeClassScope(props: { value: "compact" | "regular"; key?: string; children?: JSX.Element | null | false }): JSX.Element;

/** LIFO back-navigation handler (intercepts Back only, never Home). */
export declare function useBackHandler(enabled: boolean, onBack: () => void): void;

/** Nearest-rank percentiles, in ms. */
export interface Percentiles { readonly p50: number; readonly p95: number; readonly p99: number; readonly max: number }
/**
 * Frames this page showed in a window. A frame time is the gap between two frames shown back to
 * back; a frame after idle has none. `dropped` counts skipped display refreshes, `jank` frame
 * times over twice the refresh interval. `null` where the window holds no sample.
 */
export interface FrameStats {
  readonly windowMs: number; readonly frames: number; readonly samples: number;
  readonly refreshMs: number | null; readonly frameTimeMs: Percentiles | null; readonly fps: number | null;
  readonly dropped: number; readonly jank: number; readonly late: number;
  readonly cpuMs: Percentiles | null; readonly queueMs: Percentiles | null; readonly gpuMs: Percentiles | null;
  readonly inputToPresent: { readonly count: number; readonly lastMs: number | null; readonly statsMs: Percentiles | null };
  readonly firstPresentMs: number | null; readonly firstPresentReason: string | null; readonly reveals: number;
  /** Each reveal's wait for a frame, over the window. */
  readonly revealMs: Percentiles | null;
}
/** Measure this page's frames for `ms` (at most 60000) from now; resolves when the window ends. */
export declare function measureFrames(ms: number): Promise<FrameStats>;
/** A `className` value `cx` joins: falsy parts drop out; a map keeps the keys whose value is truthy. */
export type ClassValue = string | number | false | null | undefined | readonly ClassValue[] | { readonly [className: string]: unknown };
/** Joins class text, as `clsx`. The build checks every class it can see in the arguments. */
export declare function cx(...values: ClassValue[]): string;

export declare function List<T>(props: DataListProps<T>): JSX.Element;
export interface BoxShadow { offsetX?: number; offsetY?: number; blur?: number; spread?: number; color?: string }
/** Strokes around the box (`outer`, after an `offset` gap) and inside its border (`inner`); they never take layout space. */
export interface Ring {
  outer?: { width: number; offset?: number; color?: string; offsetColor?: string };
  inner?: { width: number; color?: string };
}
export interface GradientStop { offset: number; color: string }
/** Positions are fractions of the box; conic angles are degrees clockwise from the top. */
export type BackgroundGradient =
  | { kind?: "linear"; direction?: "top" | "right" | "bottom" | "left" | "top-right" | "bottom-right" | "bottom-left" | "top-left"; angle?: number; colorSpace?: "srgb" | "oklab"; stops: ReadonlyArray<GradientStop> }
  | { kind: "radial"; shape?: "circle" | "ellipse"; size?: "closest-side" | "closest-corner" | "farthest-side" | "farthest-corner"; at?: readonly [number, number]; colorSpace?: "srgb" | "oklab"; stops: ReadonlyArray<GradientStop> }
  | { kind: "conic"; from?: number; at?: readonly [number, number]; colorSpace?: "srgb" | "oklab"; stops: ReadonlyArray<GradientStop> };
export interface PieLayout { center: readonly [number, number]; innerRadius: number; outerRadius: number; emphasisScale?: number }
/** `fill`/`stroke` take a colour role (`'line-strong/65'`), a hex color, `currentColor`, `none` or `url(#id)` naming one of the vector's `gradients`. */
export interface VectorPath {
  /** Pairs this path with the previous source's path of the same key for `transition-[d]`; unkeyed paths pair by position. */
  key?: string;
  /** What the path's whole length counts as for `strokeDasharray`/`strokeDashoffset`: with `1`, a dash of `[1]` offset from 1 to 0 under `transition-[d]` draws the line in. */
  pathLength?: number;
  d: string; fill?: string; stroke?: string; strokeWidth?: number; strokeDasharray?: readonly number[]; strokeDashoffset?: number;
  strokeLinecap?: "butt" | "round" | "square"; strokeLinejoin?: "miter" | "round" | "bevel";
  /** An SVG transform list: matrix, translate, scale, rotate, skewX, skewY. */
  transform?: string; vectorEffect?: "none" | "non-scaling-stroke";
}
/** SVG gradient; coordinates default to `objectBoundingBox` units. */
export interface VectorGradient {
  type: "linear" | "radial" | "conic"; x1?: number; y1?: number; x2?: number; y2?: number;
  cx?: number; cy?: number; r?: number; fx?: number; fy?: number; fr?: number;
  /** Conic start angle, degrees clockwise from the top. */
  from?: number;
  units?: "objectBoundingBox" | "userSpaceOnUse"; spread?: "pad" | "reflect" | "repeat";
  stops: ReadonlyArray<{ offset: number; color: string; opacity?: number }>;
}
/** One choice of a `<radio>` or `<select>`. */
export interface ChoiceOption { readonly value: string; readonly label: string; readonly disabled?: boolean; readonly "data-testid"?: string }
/** HTML autocomplete tokens; Android maps them to autofill hints. */
export type AutoComplete = "off" | "on" | "username" | "current-password" | "new-password" | "one-time-code" | "email" | "tel" | "name" | "given-name" | "family-name" | "street-address" | "postal-code" | "country" | "cc-number" | "cc-exp" | "cc-csc" | "bday";

/** The page's environment (`view.env`): read-only, and a change re-renders the page. */
export interface Env {
  readonly theme: "light" | "dark";
  /** The page surface in logical pixels. */
  readonly viewport: Readonly<{ width: number; height: number }>;
  /** System bars and host chrome over the surface, logical pixels. */
  readonly insets: Readonly<Edges>;
  /** The system bars alone (status bar, cutout, home indicator), as CSS `env(safe-area-inset-*)`; a dialog covering host chrome pads by these, logical pixels. */
  readonly safeArea: Readonly<Edges>;
  /** Physical display hinge/crease intersecting this page canvas in local logical pixels; null when absent. */
  readonly hinge: Readonly<HingeRect> | null;
  readonly keyboard: Readonly<{ height: number }>;
  /** The host's capsule (the ··· and close control) where it floats over the page, in page coordinates, logical pixels; `null` when the host shows none or it sits in a nav bar. The same box Web pages read as `--lx-page-chrome-capsule-*`. */
  readonly capsule: Readonly<{ x: number; y: number; width: number; height: number }> | null;
  /** `fine` pointers (mouse, trackpad) hover; `coarse` ones (touch) do not. */
  readonly pointer: "fine" | "coarse";
  /** The host OS, the same value as Logic's `lx.host.info.os` and `lx.device.info.osName`. */
  readonly os: "ios" | "macos" | "android" | "windows" | "harmony" | "unknown";
  /** `compact` below 600 logical pixels of page width, else `regular`, with 24 pixels of hysteresis: compact holds until 624, regular until below 576. Near the edge it can disagree with `min-[600px]:`; use `sizeClass` for structure and `min-[…]:` for spacing and type. */
  readonly sizeClass: "compact" | "regular";
  /** The language the page's text is in: the host's display language narrowed to the `languages` its lxapp.json lists. `t`, built-in words, direction and glyph forms follow it. */
  readonly language: string;
  /** The system asks for reduced motion; `motion-reduce:`/`motion-safe:` follow it. */
  readonly reducedMotion: boolean;
  /** The system asks for increased contrast; the palette resolves its high-contrast variant. */
  readonly highContrast: boolean;
  /** The platform's animation duration multiplier (0..10, default 1); 0 also sets `reducedMotion`. */
  readonly animatorDurationScale: number;
  /** 12- or 24-hour clock: the user's system setting, else the custom of the system language. */
  readonly hourCycle: "h12" | "h23";
  /** The first day of the week, a `Weekday` (1 = Monday … 7 = Sunday): the user's system setting, else the custom of the system language. */
  readonly firstWeekday: Weekday;
  /** Inline direction derived from `language`. */
  readonly direction: "ltr" | "rtl";
  /** Whether the window can dock an `as: 'aside'` beside the main; false until the host reports it. */
  readonly canDock: boolean;
  /** `fallback: "driver"` when this device's GPU driver failed and pages render on the CPU. */
  readonly renderer?: Readonly<{ fallback: "driver" | null }>;
  /** How much the system's text size grows body text: 1 at the default size, 2 when it is twice as large, after the lxapp's `maxTextScale`. Sizes already follow it; read it only for a layout that must change shape, such as fewer columns. */
  readonly textScale: number;
}

/** A theme colour role; it resolves per appearance, so `dark:` is never needed on one. */
export type ColorRole = "canvas" | "surface" | "surface-raised" | "surface-sunken" | "line" | "text" | "text-muted" | "text-faint" | "accent" | "on-accent" | "accent-soft" | "on-accent-soft" | "danger" | "on-danger" | "danger-soft" | "success" | "on-success" | "success-soft" | "warning" | "on-warning" | "warning-soft" | "focus" | "scrim" | "chart-1" | "chart-2" | "chart-3" | "chart-4" | "chart-5" | "line-strong" | "selection" | "shadow" | "media" | "on-media" | "surface-overlay" | "knob" | "success-mark" | "warning-mark" | "danger-mark" | "media-canvas" | "info" | "on-info" | "info-soft" | "info-mark" | "hue-1" | "hue-2" | "hue-3" | "hue-4" | "hue-5" | "hue-6" | "hue-7" | "hue-8" | "hue-1-soft" | "hue-2-soft" | "hue-3-soft" | "hue-4-soft" | "hue-5-soft" | "hue-6-soft" | "hue-7-soft" | "hue-8-soft" | "chart-6" | "chart-7" | "chart-8" | "accent-solid" | "on-accent-solid" | "danger-solid" | "on-danger-solid" | "accent-solid-hover" | "accent-solid-pressed" | "danger-solid-hover" | "danger-solid-pressed" | "neutral-soft";
/** A role at an alpha percent, 0 to 100 (`'accent/12'`). */
export type ColorAlpha = `${ColorRole}/${number}`;
/** A page colour: a role, a role at an alpha percent, or `'transparent'`. */
export type ColorValue = ColorRole | ColorAlpha | "transparent";
/** A `<plot>` series colour: a role, usually `'chart-1'` to `'chart-8'`, or one at an alpha percent. */
export type ColorSeries = ColorRole | ColorAlpha;

/** CSS-named style; each element accepts the subset its `*Style` type lists. */
export interface Style {
  /** Width in logical pixels, a percent of the parent, a viewport unit, `ch`, or an intrinsic size. */
  width?: number | `${number}%` | `${number}vw` | `${number}vh` | `${number}ch` | "auto" | "fit-content" | "max-content" | "min-content";
  /** Height in logical pixels, a percent of the parent, a viewport unit, or an intrinsic size. */
  height?: number | `${number}%` | `${number}vw` | `${number}vh` | "auto" | "fit-content" | "max-content" | "min-content";
  /** The smallest width layout may give it. */
  minWidth?: number | `${number}%` | `${number}vw` | `${number}vh` | `${number}ch` | "fit-content" | "max-content" | "min-content";
  /** The smallest height layout may give it. */
  minHeight?: number | `${number}%` | `${number}vw` | `${number}vh` | "fit-content" | "max-content" | "min-content";
  /** The largest width layout may give it. */
  maxWidth?: number | `${number}%` | `${number}vw` | `${number}vh` | `${number}ch` | "fit-content" | "max-content" | "min-content";
  /** Width divided by height, kept when only one of them is set. */
  aspectRatio?: number | "auto";
  /** The largest height layout may give it. */
  maxHeight?: number | `${number}%` | `${number}vw` | `${number}vh` | "fit-content" | "max-content" | "min-content";
  /** Space outside the border on every side; `auto` takes the free space. */
  margin?: number | "auto";
  /** Space outside the top border. */
  marginTop?: number | "auto";
  /** Space outside the right border. */
  marginRight?: number | "auto";
  /** Space outside the bottom border. */
  marginBottom?: number | "auto";
  /** Space outside the left border. */
  marginLeft?: number | "auto";
  /** Space outside the start border: left in left-to-right text, right in right-to-left. */
  marginInlineStart?: number | "auto";
  /** Space outside the end border: right in left-to-right text, left in right-to-left. */
  marginInlineEnd?: number | "auto";
  /** `relative` offsets it from its place; `absolute` places it in its parent; `fixed` in the screen; `sticky` sticks inside its scroll. */
  position?: "relative" | "absolute" | "fixed" | "sticky";
  /** Offset of every edge from the containing box, for a positioned element. */
  inset?: number | `${number}%`;
  /** Offset of the top edge; `anchor(center)` centres an anchored popover on its anchor. */
  top?: number | `${number}%` | "anchor(center)";
  /** Offset of the right edge; `anchor(center)` centres an anchored popover on its anchor. */
  right?: number | `${number}%` | "anchor(center)";
  /** Offset of the bottom edge; `anchor(center)` centres an anchored popover on its anchor. */
  bottom?: number | `${number}%` | "anchor(center)";
  /** Offset of the left edge; `anchor(center)` centres an anchored popover on its anchor. */
  left?: number | `${number}%` | "anchor(center)";
  /** Offset of the start edge, following the text direction. */
  insetInlineStart?: number | `${number}%` | "anchor(center)";
  /** Offset of the end edge, following the text direction. */
  insetInlineEnd?: number | `${number}%` | "anchor(center)";
  /** Paint order among siblings; higher paints over lower. */
  zIndex?: number;
  /** Where an anchored popover sits against its anchor; it flips when that side lacks room. */
  positionArea?: "bottom span-inline-end" | "bottom span-inline-start" | "bottom" | "top span-inline-end" | "top span-inline-start" | "top" | "inline-end span-block-end" | "inline-end span-block-start" | "inline-end" | "inline-start span-block-end" | "inline-start span-block-start" | "inline-start";
  /** Share of the free space along the main axis it takes. */
  flexGrow?: number;
  /** Share of the overflow along the main axis it gives up; 0 keeps its size. */
  flexShrink?: number;
  /** Main-axis size before free space is shared out. */
  flexBasis?: number | `${number}%` | "auto";
  /** Cross-axis alignment of this item, overriding its parent's `alignItems`. */
  alignSelf?: "flex-start" | "start" | "center" | "flex-end" | "end" | "stretch" | "baseline" | "safe center" | "safe flex-end" | "safe end";
  /** Grid columns it spans, as `start / end` or `span n`. */
  gridColumn?: number | string;
  /** Grid line its column span starts at. */
  gridColumnStart?: number | string;
  /** Grid line its column span ends at, or `span n`. */
  gridColumnEnd?: number | string;
  /** Grid rows it spans, as `start / end` or `span n`. */
  gridRow?: number | string;
  /** Grid line its row span starts at. */
  gridRowStart?: number | string;
  /** Grid line its row span ends at, or `span n`. */
  gridRowEnd?: number | string;
  /** Inline alignment of this grid item in its cell. */
  justifySelf?: "start" | "center" | "end" | "stretch" | "baseline" | "safe center" | "safe end";
  /** Layout order among flex or grid siblings; lower comes first. */
  order?: number;
  /** Opacity of the element and its subtree, 0..1. */
  opacity?: number;
  /** `hidden` clips the subtree to the border box and its radius. */
  overflow?: "visible" | "hidden";
  /** Fill behind the content, as a colour role. */
  backgroundColor?: ColorValue | "currentColor";
  /** A linear, radial or conic gradient painted behind the content. */
  backgroundImage?: BackgroundGradient | "none";
  /** Corner radius of every corner. */
  borderRadius?: number;
  /** Radius of the top-left corner. */
  borderTopLeftRadius?: number;
  /** Radius of the top-right corner. */
  borderTopRightRadius?: number;
  /** Radius of the bottom-right corner. */
  borderBottomRightRadius?: number;
  /** Radius of the bottom-left corner. */
  borderBottomLeftRadius?: number;
  /** Radius of the top corner on the start side. */
  borderStartStartRadius?: number;
  /** Radius of the top corner on the end side. */
  borderStartEndRadius?: number;
  /** Radius of the bottom corner on the start side. */
  borderEndStartRadius?: number;
  /** Radius of the bottom corner on the end side. */
  borderEndEndRadius?: number;
  /** Border width on every side. */
  borderWidth?: number;
  /** Width of the top border. */
  borderTopWidth?: number;
  /** Width of the right border. */
  borderRightWidth?: number;
  /** Width of the bottom border. */
  borderBottomWidth?: number;
  /** Width of the left border. */
  borderLeftWidth?: number;
  /** Width of the start-side border, following the text direction. */
  borderInlineStartWidth?: number;
  /** Width of the end-side border, following the text direction. */
  borderInlineEndWidth?: number;
  /** Line style of every border. */
  borderStyle?: "solid" | "dashed" | "dotted";
  /** Colour of every border, as a colour role. */
  borderColor?: ColorValue | "currentColor";
  /** Shadows painted outside the border box, up to the first listed on top. */
  boxShadow?: ReadonlyArray<BoxShadow> | "none";
  /** A focus-style outline: an outer ring with an optional gap, and an inner ring. */
  ring?: Ring | "none";
  /** Filter applied to what is behind it, such as `blur(12px)` for frosted glass. */
  backdropFilter?: string;
  /** Moves it without changing layout, in pixels or percents of its own size. */
  translate?: number | `${number}%` | readonly [number | `${number}%`, number | `${number}%`];
  /** Rotation in degrees around `transformOrigin`, without changing layout. */
  rotate?: number;
  /** Scale, uniform or `[x, y]`, around `transformOrigin`, without changing layout. */
  scale?: number | readonly [number, number];
  /** The point `rotate` and `scale` turn around, as fractions `[x, y]` of its box. */
  transformOrigin?: readonly [number, number];
  /** Filter applied to its own pixels, such as `blur(4px)` or `grayscale(1)`. */
  filter?: string;
  /** Space inside the border on every side. */
  padding?: number;
  /** Space inside the top border. */
  paddingTop?: number;
  /** Space inside the right border. */
  paddingRight?: number;
  /** Space inside the bottom border. */
  paddingBottom?: number;
  /** Space inside the left border. */
  paddingLeft?: number;
  /** Space inside the start border, following the text direction. */
  paddingInlineStart?: number;
  /** Space inside the end border, following the text direction. */
  paddingInlineEnd?: number;
  /** `flex` (the default) or `grid` lays out children; `none` removes it from layout. */
  display?: "flex" | "grid" | "none" | "block" | "inline-flex" | "inline-block";
  /** `none` keeps presses to its own box instead of widening it to the platform touch target. */
  touchTarget?: "auto" | "none";
  /** Main axis of a flex container; `<view>` is a column unless set. */
  flexDirection?: "row" | "column" | "row-reverse" | "column-reverse";
  /** Cross-axis alignment of the children. */
  alignItems?: "flex-start" | "start" | "center" | "flex-end" | "end" | "stretch" | "baseline" | "safe center" | "safe flex-end" | "safe end";
  /** Main-axis placement of the children and the space between them. */
  justifyContent?: "flex-start" | "start" | "center" | "flex-end" | "end" | "space-between" | "space-around" | "space-evenly" | "safe center" | "safe flex-end" | "safe end";
  /** Placement of wrapped lines or grid tracks in the cross axis. */
  alignContent?: "flex-start" | "start" | "center" | "flex-end" | "end" | "space-between" | "space-around" | "space-evenly" | "stretch" | "safe center" | "safe flex-end" | "safe end";
  /** Whether children wrap onto more lines when they do not fit. */
  flexWrap?: "nowrap" | "wrap" | "wrap-reverse";
  /** Space between children in both axes. */
  gap?: number;
  /** Space between rows. */
  rowGap?: number;
  /** Space between columns. */
  columnGap?: number;
  /** Column tracks of a grid: a count of equal columns, or a track list. */
  gridTemplateColumns?: number | string;
  /** Row tracks of a grid: a count of equal rows, or a track list. */
  gridTemplateRows?: number | string;
  /** Size of columns the grid creates on its own. */
  gridAutoColumns?: string;
  /** Size of rows the grid creates on its own. */
  gridAutoRows?: string;
  /** Direction items without a place fill the grid in; `dense` fills earlier holes. */
  gridAutoFlow?: "row" | "column" | "row dense" | "column dense";
  /** Inline alignment of every grid item in its cell. */
  justifyItems?: "start" | "center" | "end" | "stretch" | "baseline" | "safe center" | "safe end";
  /** Text and icon colour, as a colour role; inherited. */
  color?: ColorValue;
  /** Font size in logical pixels, scaled by the system text size; inherited. */
  fontSize?: number;
  /** Font weight, 1..1000; inherited. */
  fontWeight?: number;
  /** A generic family or a font the app declares; inherited. */
  fontFamily?: "sans-serif" | "monospace" | "serif" | (string & {});
  /** Upright or italic; inherited. */
  fontStyle?: "normal" | "italic";
  /** Line height as a multiple of the font size, or in pixels; inherited. */
  lineHeight?: number | `${number}px`;
  /** Extra space between letters, in pixels or `em`; inherited. */
  letterSpacing?: number | `${number}em`;
  /** Numeric glyph variants such as `tabular-nums`; inherited. */
  fontVariantNumeric?: string;
  /** Whether its text can be selected and copied; inherited. */
  userSelect?: "none" | "text" | "all";
  /** Horizontal alignment of lines; `start` and `end` follow the text direction. */
  textAlign?: "start" | "end" | "left" | "center" | "right" | "justify";
  /** Underline or strike-through. */
  textDecorationLine?: "none" | "line-through" | "underline";
  /** Line style of the decoration. */
  textDecorationStyle?: "solid" | "dashed" | "dotted";
  /** Colour of the decoration, as a colour role. */
  textDecorationColor?: ColorValue | "currentColor";
  /** Case change applied when shown; the text itself is unchanged. */
  textTransform?: "none" | "uppercase" | "lowercase" | "capitalize";
  /** `text` paints the background through the glyphs only. */
  backgroundClip?: "border-box" | "text";
  /** How spaces and newlines collapse and whether lines wrap. */
  whiteSpace?: "normal" | "nowrap" | "pre" | "pre-wrap" | "pre-line";
  /** Where words may break: anywhere (`break-all`), or never inside CJK runs (`keep-all`). */
  wordBreak?: "normal" | "break-all" | "keep-all";
  /** Whether a word longer than the line breaks inside rather than overflowing. */
  overflowWrap?: "normal" | "break-word" | "anywhere";
  /** Lines shown at most; the rest is cut with an ellipsis. */
  lineClamp?: number;
  /** `ellipsis` ends a cut line with an ellipsis. */
  textOverflow?: "clip" | "ellipsis";
  /** `auto` scrolls a `<scroll>` horizontally. */
  overflowX?: "auto";
  /** `auto` scrolls a `<scroll>` vertically. */
  overflowY?: "auto";
  /** `contain` keeps a scroll at its edge from moving an outer one; `none` also drops the bounce. */
  overscrollBehavior?: "auto" | "contain" | "none";
  /** `always` stops a swipe at each snap point instead of skipping past. */
  scrollSnapStop?: "normal" | "always";
  /** Axis a scroll snaps along, to its children's `scrollSnapAlign`. */
  scrollSnapType?: "none" | "x" | "y";
  /** Edge of this child a snapping scroll rests on. */
  scrollSnapAlign?: "none" | "start" | "center" | "end";
  /** Colour of an editor's placeholder, as a colour role. */
  placeholderColor?: ColorValue;
  /** How the picture fills its box: `cover` crops, `contain` letterboxes, `fill` stretches. */
  objectFit?: "cover" | "contain" | "fill";
  /** Where the picture sits in its box when it does not fill it, such as `top` or `25% 75%`. */
  objectPosition?: string;
}

/** Style keys by group; each element's `*Style` joins the groups it accepts. */
export namespace StyleGroup {
  /** Size, margin, position, placement as an item, opacity, background, border, shadow, filters and transforms. */
  export interface Box {
    /** Width in logical pixels, a percent of the parent, a viewport unit, `ch`, or an intrinsic size. */
    width?: number | `${number}%` | `${number}vw` | `${number}vh` | `${number}ch` | "auto" | "fit-content" | "max-content" | "min-content";
    /** Height in logical pixels, a percent of the parent, a viewport unit, or an intrinsic size. */
    height?: number | `${number}%` | `${number}vw` | `${number}vh` | "auto" | "fit-content" | "max-content" | "min-content";
    /** The smallest width layout may give it. */
    minWidth?: number | `${number}%` | `${number}vw` | `${number}vh` | `${number}ch` | "fit-content" | "max-content" | "min-content";
    /** The smallest height layout may give it. */
    minHeight?: number | `${number}%` | `${number}vw` | `${number}vh` | "fit-content" | "max-content" | "min-content";
    /** The largest width layout may give it. */
    maxWidth?: number | `${number}%` | `${number}vw` | `${number}vh` | `${number}ch` | "fit-content" | "max-content" | "min-content";
    /** Width divided by height, kept when only one of them is set. */
    aspectRatio?: number | "auto";
    /** The largest height layout may give it. */
    maxHeight?: number | `${number}%` | `${number}vw` | `${number}vh` | "fit-content" | "max-content" | "min-content";
    /** Space outside the border on every side; `auto` takes the free space. */
    margin?: number | "auto";
    /** Space outside the top border. */
    marginTop?: number | "auto";
    /** Space outside the right border. */
    marginRight?: number | "auto";
    /** Space outside the bottom border. */
    marginBottom?: number | "auto";
    /** Space outside the left border. */
    marginLeft?: number | "auto";
    /** Space outside the start border: left in left-to-right text, right in right-to-left. */
    marginInlineStart?: number | "auto";
    /** Space outside the end border: right in left-to-right text, left in right-to-left. */
    marginInlineEnd?: number | "auto";
    /** `relative` offsets it from its place; `absolute` places it in its parent; `fixed` in the screen; `sticky` sticks inside its scroll. */
    position?: "relative" | "absolute" | "fixed" | "sticky";
    /** Offset of every edge from the containing box, for a positioned element. */
    inset?: number | `${number}%`;
    /** Offset of the top edge; `anchor(center)` centres an anchored popover on its anchor. */
    top?: number | `${number}%` | "anchor(center)";
    /** Offset of the right edge; `anchor(center)` centres an anchored popover on its anchor. */
    right?: number | `${number}%` | "anchor(center)";
    /** Offset of the bottom edge; `anchor(center)` centres an anchored popover on its anchor. */
    bottom?: number | `${number}%` | "anchor(center)";
    /** Offset of the left edge; `anchor(center)` centres an anchored popover on its anchor. */
    left?: number | `${number}%` | "anchor(center)";
    /** Offset of the start edge, following the text direction. */
    insetInlineStart?: number | `${number}%` | "anchor(center)";
    /** Offset of the end edge, following the text direction. */
    insetInlineEnd?: number | `${number}%` | "anchor(center)";
    /** Paint order among siblings; higher paints over lower. */
    zIndex?: number;
    /** Where an anchored popover sits against its anchor; it flips when that side lacks room. */
    positionArea?: "bottom span-inline-end" | "bottom span-inline-start" | "bottom" | "top span-inline-end" | "top span-inline-start" | "top" | "inline-end span-block-end" | "inline-end span-block-start" | "inline-end" | "inline-start span-block-end" | "inline-start span-block-start" | "inline-start";
    /** Share of the free space along the main axis it takes. */
    flexGrow?: number;
    /** Share of the overflow along the main axis it gives up; 0 keeps its size. */
    flexShrink?: number;
    /** Main-axis size before free space is shared out. */
    flexBasis?: number | `${number}%` | "auto";
    /** Cross-axis alignment of this item, overriding its parent's `alignItems`. */
    alignSelf?: "flex-start" | "start" | "center" | "flex-end" | "end" | "stretch" | "baseline" | "safe center" | "safe flex-end" | "safe end";
    /** Grid columns it spans, as `start / end` or `span n`. */
    gridColumn?: number | string;
    /** Grid line its column span starts at. */
    gridColumnStart?: number | string;
    /** Grid line its column span ends at, or `span n`. */
    gridColumnEnd?: number | string;
    /** Grid rows it spans, as `start / end` or `span n`. */
    gridRow?: number | string;
    /** Grid line its row span starts at. */
    gridRowStart?: number | string;
    /** Grid line its row span ends at, or `span n`. */
    gridRowEnd?: number | string;
    /** Inline alignment of this grid item in its cell. */
    justifySelf?: "start" | "center" | "end" | "stretch" | "baseline" | "safe center" | "safe end";
    /** Layout order among flex or grid siblings; lower comes first. */
    order?: number;
    /** Opacity of the element and its subtree, 0..1. */
    opacity?: number;
    /** `hidden` clips the subtree to the border box and its radius. */
    overflow?: "visible" | "hidden";
    /** Fill behind the content, as a colour role. */
    backgroundColor?: ColorValue | "currentColor";
    /** A linear, radial or conic gradient painted behind the content. */
    backgroundImage?: BackgroundGradient | "none";
    /** Corner radius of every corner. */
    borderRadius?: number;
    /** Radius of the top-left corner. */
    borderTopLeftRadius?: number;
    /** Radius of the top-right corner. */
    borderTopRightRadius?: number;
    /** Radius of the bottom-right corner. */
    borderBottomRightRadius?: number;
    /** Radius of the bottom-left corner. */
    borderBottomLeftRadius?: number;
    /** Radius of the top corner on the start side. */
    borderStartStartRadius?: number;
    /** Radius of the top corner on the end side. */
    borderStartEndRadius?: number;
    /** Radius of the bottom corner on the start side. */
    borderEndStartRadius?: number;
    /** Radius of the bottom corner on the end side. */
    borderEndEndRadius?: number;
    /** Border width on every side. */
    borderWidth?: number;
    /** Width of the top border. */
    borderTopWidth?: number;
    /** Width of the right border. */
    borderRightWidth?: number;
    /** Width of the bottom border. */
    borderBottomWidth?: number;
    /** Width of the left border. */
    borderLeftWidth?: number;
    /** Width of the start-side border, following the text direction. */
    borderInlineStartWidth?: number;
    /** Width of the end-side border, following the text direction. */
    borderInlineEndWidth?: number;
    /** Line style of every border. */
    borderStyle?: "solid" | "dashed" | "dotted";
    /** Colour of every border, as a colour role. */
    borderColor?: ColorValue | "currentColor";
    /** Shadows painted outside the border box, up to the first listed on top. */
    boxShadow?: ReadonlyArray<BoxShadow> | "none";
    /** A focus-style outline: an outer ring with an optional gap, and an inner ring. */
    ring?: Ring | "none";
    /** Filter applied to what is behind it, such as `blur(12px)` for frosted glass. */
    backdropFilter?: string;
    /** Moves it without changing layout, in pixels or percents of its own size. */
    translate?: number | `${number}%` | readonly [number | `${number}%`, number | `${number}%`];
    /** Rotation in degrees around `transformOrigin`, without changing layout. */
    rotate?: number;
    /** Scale, uniform or `[x, y]`, around `transformOrigin`, without changing layout. */
    scale?: number | readonly [number, number];
    /** The point `rotate` and `scale` turn around, as fractions `[x, y]` of its box. */
    transformOrigin?: readonly [number, number];
    /** Filter applied to its own pixels, such as `blur(4px)` or `grayscale(1)`. */
    filter?: string;
    /** `flex` (the default) or `grid` lays out children; `none` removes it from layout. */
    display?: "flex" | "grid" | "none" | "block" | "inline-flex" | "inline-block";
    /** `none` keeps presses to its own box instead of widening it to the platform touch target. */
    touchTarget?: "auto" | "none";
    /** Edge of this child a snapping scroll rests on. */
    scrollSnapAlign?: "none" | "start" | "center" | "end";
  }
  /** Padding, physical and flow-relative. */
  export interface Padding {
    /** Space inside the border on every side. */
    padding?: number;
    /** Space inside the top border. */
    paddingTop?: number;
    /** Space inside the right border. */
    paddingRight?: number;
    /** Space inside the bottom border. */
    paddingBottom?: number;
    /** Space inside the left border. */
    paddingLeft?: number;
    /** Space inside the start border, following the text direction. */
    paddingInlineStart?: number;
    /** Space inside the end border, following the text direction. */
    paddingInlineEnd?: number;
  }
  /** Whether a flex container wraps. */
  export interface Flex {
    /** Whether children wrap onto more lines when they do not fit. */
    flexWrap?: "nowrap" | "wrap" | "wrap-reverse";
  }
  /** How a container aligns its children. */
  export interface Align {
    /** Cross-axis alignment of the children. */
    alignItems?: "flex-start" | "start" | "center" | "flex-end" | "end" | "stretch" | "baseline" | "safe center" | "safe flex-end" | "safe end";
    /** Main-axis placement of the children and the space between them. */
    justifyContent?: "flex-start" | "start" | "center" | "flex-end" | "end" | "space-between" | "space-around" | "space-evenly" | "safe center" | "safe flex-end" | "safe end";
    /** Placement of wrapped lines or grid tracks in the cross axis. */
    alignContent?: "flex-start" | "start" | "center" | "flex-end" | "end" | "space-between" | "space-around" | "space-evenly" | "stretch" | "safe center" | "safe flex-end" | "safe end";
  }
  /** The main axis of a flex container. */
  export interface Direction {
    /** Main axis of a flex container; `<view>` is a column unless set. */
    flexDirection?: "row" | "column" | "row-reverse" | "column-reverse";
  }
  /** Space between children. */
  export interface Gap {
    /** Space between children in both axes. */
    gap?: number;
    /** Space between rows. */
    rowGap?: number;
    /** Space between columns. */
    columnGap?: number;
  }
  /** Text colour, font, line height, spacing, alignment, decoration, wrapping and selection; inherited. */
  export interface Text {
    /** Text and icon colour, as a colour role; inherited. */
    color?: ColorValue;
    /** Font size in logical pixels, scaled by the system text size; inherited. */
    fontSize?: number;
    /** Font weight, 1..1000; inherited. */
    fontWeight?: number;
    /** A generic family or a font the app declares; inherited. */
    fontFamily?: "sans-serif" | "monospace" | "serif" | (string & {});
    /** Upright or italic; inherited. */
    fontStyle?: "normal" | "italic";
    /** Line height as a multiple of the font size, or in pixels; inherited. */
    lineHeight?: number | `${number}px`;
    /** Extra space between letters, in pixels or `em`; inherited. */
    letterSpacing?: number | `${number}em`;
    /** Numeric glyph variants such as `tabular-nums`; inherited. */
    fontVariantNumeric?: string;
    /** Whether its text can be selected and copied; inherited. */
    userSelect?: "none" | "text" | "all";
    /** Horizontal alignment of lines; `start` and `end` follow the text direction. */
    textAlign?: "start" | "end" | "left" | "center" | "right" | "justify";
    /** Underline or strike-through. */
    textDecorationLine?: "none" | "line-through" | "underline";
    /** Line style of the decoration. */
    textDecorationStyle?: "solid" | "dashed" | "dotted";
    /** Colour of the decoration, as a colour role. */
    textDecorationColor?: ColorValue | "currentColor";
    /** Case change applied when shown; the text itself is unchanged. */
    textTransform?: "none" | "uppercase" | "lowercase" | "capitalize";
    /** How spaces and newlines collapse and whether lines wrap. */
    whiteSpace?: "normal" | "nowrap" | "pre" | "pre-wrap" | "pre-line";
    /** Where words may break: anywhere (`break-all`), or never inside CJK runs (`keep-all`). */
    wordBreak?: "normal" | "break-all" | "keep-all";
    /** Whether a word longer than the line breaks inside rather than overflowing. */
    overflowWrap?: "normal" | "break-word" | "anywhere";
  }
  /** Grid tracks and auto placement. */
  export interface Grid {
    /** Column tracks of a grid: a count of equal columns, or a track list. */
    gridTemplateColumns?: number | string;
    /** Row tracks of a grid: a count of equal rows, or a track list. */
    gridTemplateRows?: number | string;
    /** Size of columns the grid creates on its own. */
    gridAutoColumns?: string;
    /** Size of rows the grid creates on its own. */
    gridAutoRows?: string;
    /** Direction items without a place fill the grid in; `dense` fills earlier holes. */
    gridAutoFlow?: "row" | "column" | "row dense" | "column dense";
    /** Inline alignment of every grid item in its cell. */
    justifyItems?: "start" | "center" | "end" | "stretch" | "baseline" | "safe center" | "safe end";
  }
  /** Line clamping, overflow and clipping a background to the text. */
  export interface TextBlock {
    /** `text` paints the background through the glyphs only. */
    backgroundClip?: "border-box" | "text";
    /** Lines shown at most; the rest is cut with an ellipsis. */
    lineClamp?: number;
    /** `ellipsis` ends a cut line with an ellipsis. */
    textOverflow?: "clip" | "ellipsis";
  }
  /** Which axes scroll. */
  export interface Scroll {
    /** `auto` scrolls a `<scroll>` horizontally. */
    overflowX?: "auto";
    /** `auto` scrolls a `<scroll>` vertically. */
    overflowY?: "auto";
  }
  /** Overscroll hand-off and snapping. */
  export interface Overscroll {
    /** `contain` keeps a scroll at its edge from moving an outer one; `none` also drops the bounce. */
    overscrollBehavior?: "auto" | "contain" | "none";
    /** `always` stops a swipe at each snap point instead of skipping past. */
    scrollSnapStop?: "normal" | "always";
    /** Axis a scroll snaps along, to its children's `scrollSnapAlign`. */
    scrollSnapType?: "none" | "x" | "y";
  }
  /** How an image or video fits its box. */
  export interface Media {
    /** How the picture fills its box: `cover` crops, `contain` letterboxes, `fill` stretches. */
    objectFit?: "cover" | "contain" | "fill";
  }
  /** Where a fitted image or video sits in its box. */
  export interface ImagePlacement {
    /** Where the picture sits in its box when it does not fill it, such as `top` or `25% 75%`. */
    objectPosition?: string;
  }
  /** The placeholder colour of an editor. */
  export interface Editor {
    /** Colour of an editor's placeholder, as a colour role. */
    placeholderColor?: ColorValue;
  }
  /** How cut text ends. */
  export interface Ellipsis {
    /** `ellipsis` ends a cut line with an ellipsis. */
    textOverflow?: "clip" | "ellipsis";
  }
  /** The accent colour of a control. */
  export interface Accent {
    /** Text and icon colour, as a colour role; inherited. */
    color?: ColorValue;
  }
  /** Colour, font and decoration of a run inside text. */
  export interface Run {
    /** Text and icon colour, as a colour role; inherited. */
    color?: ColorValue;
    /** Font size in logical pixels, scaled by the system text size; inherited. */
    fontSize?: number;
    /** Font weight, 1..1000; inherited. */
    fontWeight?: number;
    /** A generic family or a font the app declares; inherited. */
    fontFamily?: "sans-serif" | "monospace" | "serif" | (string & {});
    /** Upright or italic; inherited. */
    fontStyle?: "normal" | "italic";
    /** Extra space between letters, in pixels or `em`; inherited. */
    letterSpacing?: number | `${number}em`;
    /** Underline or strike-through. */
    textDecorationLine?: "none" | "line-through" | "underline";
    /** Line style of the decoration. */
    textDecorationStyle?: "solid" | "dashed" | "dotted";
    /** Colour of the decoration, as a colour role. */
    textDecorationColor?: ColorValue | "currentColor";
    /** Numeric glyph variants such as `tabular-nums`; inherited. */
    fontVariantNumeric?: string;
    /** Fill behind the content, as a colour role. */
    backgroundColor?: ColorValue | "currentColor";
  }
}

export type ViewStyle = StyleGroup.Box & StyleGroup.Padding & StyleGroup.Flex & StyleGroup.Align & StyleGroup.Direction & StyleGroup.Gap & StyleGroup.Text & StyleGroup.Grid;
/** The roles a `<view>` takes; reference/elements.md says where the others go. */
export type ViewRole = "heading" | "list" | "listitem" | "dialog" | "alert" | "status" | "grid" | "gridcell" | "listbox" | "option" | "menu" | "progressbar" | "group" | "tooltip" | "tablist" | "tabpanel" | "radiogroup" | "toolbar" | "tree" | "row" | "img";
export type ViewProps = Omit<Props, "style" | "children" | "role"> & { role?: ViewRole; style?: ViewStyle; children?: Child;
  /** A heading's level, 1..6, with `role="heading"`. */ headingLevel?: 1 | 2 | 3 | 4 | 5 | 6;
  /** The value of a `progressbar`, a `spinbutton` or an `onPointer` element, between `aria-valuemin` and `aria-valuemax`; leave it out while a progressbar's amount is unknown. On an `onPointer` element it is the value arrows and a screen reader step. */ "aria-valuenow"?: number;
  /** The lowest value; 0 unless set. */ "aria-valuemin"?: number;
  /** The highest value; 100 unless set. */ "aria-valuemax"?: number;
  /** Spoken in place of the number, such as "1:20 of 3:05". */ "aria-valuetext"?: string;
  /** On a popover with `role="dialog"`: modal like a `<dialog>`. Tab stays inside, the rest of the page leaves accessibility, and the page shows over the host's bars; without it the popover dialog is non-modal. */ "aria-modal"?: boolean;
  /** On a direct child of `<page>`: extend under the safe-area insets to the screen edge, on every side or the named ones. */ bleed?: BleedEdge;
  /** Called for PageUp and PageDown on a `role="grid"`. */ onPage?: (direction: "previous" | "next") => void;
  /** Called for a long press or a secondary click, with where it happened. */ onContextMenu?: (event: ContextMenuEvent) => void;
  /** Reports a single pointer over the element: down, move, up, enter, leave and cancel. */ onPointer?: (event: PointerEvent) => void;
  /** The axis `onPointer` keeps; the other is left to the page's scroll. */ pointerAxis?: "x" | "y" | "both";
  /** Called with its laid-out size after each change, one render behind. */ onLayout?: (box: BoxSize) => void;
  /** A `useBox` handle this element fills with its laid-out size. */ box?: Box;
  /** The accessible name, spoken in place of its text. */ "aria-label"?: string;
  /** Called when a mouse, stylus or trackpad pointer enters it. */ onPointerEnter?: () => void;
  /** Called when that pointer leaves it. */ onPointerLeave?: () => void;
} & ({ 
  /** Makes it a popover in the top layer: `auto` light-dismisses, `manual` closes only when told, `hint` for a hover hint. */ popover: "manual"; 
  /** Whether the popover is shown; with `onToggle` to follow its own closing. */ open?: boolean; 
  /** Called with the new state when the popover opens or closes. */ onToggle?: (open: boolean) => void; 
  /** On a `popover="manual"`: close it this many milliseconds (1..60000) after it opens, as a toast. */ closeAfter?: number; 
  /** The id of the element a popover is placed against; `<id>:caret` or `<id>:selection` follows an editor's caret or selection each frame. */ anchor?: string; 
  /** Called when Escape, back or a screen reader asks the popover to close; return `false` to keep it open. */ onCloseRequest?: never; } | { 
  /** Makes it a popover in the top layer: `auto` light-dismisses, `manual` closes only when told, `hint` for a hover hint. */ popover: true | "auto" | "hint"; 
  /** Whether the popover is shown; with `onToggle` to follow its own closing. */ open?: boolean; 
  /** Called with the new state when the popover opens or closes. */ onToggle?: (open: boolean) => void; 
  /** On a `popover="manual"`: close it this many milliseconds (1..60000) after it opens, as a toast. */ closeAfter?: number; 
  /** The id of the element a popover is placed against; `<id>:caret` or `<id>:selection` follows an editor's caret or selection each frame. */ anchor?: string; 
  /** Called when Escape, back or a screen reader asks the popover to close; return `false` to keep it open. */ onCloseRequest?: () => boolean | void; } | { 
  /** Makes it a popover in the top layer: `auto` light-dismisses, `manual` closes only when told, `hint` for a hover hint. */ popover?: false | undefined; 
  /** Whether the popover is shown; with `onToggle` to follow its own closing. */ open?: never; 
  /** Called with the new state when the popover opens or closes. */ onToggle?: never; 
  /** On a `popover="manual"`: close it this many milliseconds (1..60000) after it opens, as a toast. */ closeAfter?: never; 
  /** The id of the element a popover is placed against; `<id>:caret` or `<id>:selection` follows an editor's caret or selection each frame. */ anchor?: never; 
  /** Called when Escape, back or a screen reader asks the popover to close; return `false` to keep it open. */ onCloseRequest?: never; }) & ({ 
  /** Lets the finger move it and its subtree along one axis, between its `dragSnapPoints`. */ drag: "x" | "y"; 
  /** Up to 8 offsets, in pixels or percents of its size, a drag comes to rest at. */ dragSnapPoints?: readonly (number | `${number}%`)[]; 
  /** A number `useLocal` cell holding the index of the snap point it rests on. */ bind?: Local<number>; 
  /** The index of the snap point it rests on, with `onSnapChange`; not with `bind`. */ snapIndex?: number; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange?: (index: number) => void; 
  /** Called when a drag starts. */ onDragStart?: () => void; 
  /** Called when a drag is released, with the snap index it heads for and its velocity. */ onDragEnd?: (info: { index: number; velocity: number }) => void; 
  /** Ids of buttons (space-separated) a screen reader offers as this element's actions, as for a swipe row. */ "aria-actions"?: string; } | { 
  /** Lets the finger move it and its subtree along one axis, between its `dragSnapPoints`. */ drag?: undefined; 
  /** Up to 8 offsets, in pixels or percents of its size, a drag comes to rest at. */ dragSnapPoints?: never; 
  /** A number `useLocal` cell holding the index of the snap point it rests on. */ bind?: never; 
  /** The index of the snap point it rests on, with `onSnapChange`; not with `bind`. */ snapIndex?: never; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange?: never; 
  /** Called when a drag starts. */ onDragStart?: never; 
  /** Called when a drag is released, with the snap index it heads for and its velocity. */ onDragEnd?: never; 
  /** Ids of buttons (space-separated) a screen reader offers as this element's actions, as for a swipe row. */ "aria-actions"?: never; }) & ({ 
  /** The index of the snap point it rests on, with `onSnapChange`; not with `bind`. */ snapIndex: number; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange: (index: number) => void; 
  /** A number `useLocal` cell holding the index of the snap point it rests on. */ bind?: never; } | { 
  /** A number `useLocal` cell holding the index of the snap point it rests on. */ bind: Local<number>;  
  /** The index of the snap point it rests on, with `onSnapChange`; not with `bind`. */ snapIndex?: never; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange?: never; } | { 
  /** A number `useLocal` cell holding the index of the snap point it rests on. */ bind?: never; 
  /** The index of the snap point it rests on, with `onSnapChange`; not with `bind`. */ snapIndex?: never; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange?: never; });
export type PageStyle = StyleGroup.Box & StyleGroup.Padding & StyleGroup.Flex & StyleGroup.Align & StyleGroup.Direction & StyleGroup.Gap & StyleGroup.Text & StyleGroup.Grid;
/** The roles a `<page>` takes; reference/elements.md says where the others go. */
export type PageRole = never;
export type PageProps = Omit<Props, "style" | "children" | "role"> & { role?: PageRole; style?: PageStyle; children?: Child;
  /** The title the host navigation bar shows for the page; the page's only title writer, so Logic changes it through page data, not `navigationBar.update`. */ title?: string;
  /** The accessible name, spoken in place of its text. */ "aria-label"?: string;
};
export type TextStyle = StyleGroup.Box & StyleGroup.Padding & StyleGroup.Text & StyleGroup.TextBlock;
/** The roles a `<text>` takes; reference/elements.md says where the others go. */
export type TextRole = "heading" | "listitem" | "alert" | "status" | "gridcell" | "option" | "tooltip" | "img";
export type TextProps = Omit<Props, "style" | "children" | "role"> & { role?: TextRole; style?: TextStyle; children?: RunChild;
  /** A heading's level, 1..6, with `role="heading"`. */ headingLevel?: 1 | 2 | 3 | 4 | 5 | 6;
  /** Called for a long press or a secondary click, with where it happened. */ onContextMenu?: (event: ContextMenuEvent) => void;
  /** Called with its laid-out size after each change, one render behind. */ onLayout?: (box: BoxSize) => void;
  /** A `useBox` handle this element fills with its laid-out size. */ box?: Box;
  /** The accessible name, spoken in place of its text. */ "aria-label"?: string;
};
export type ButtonStyle = StyleGroup.Box & StyleGroup.Padding & StyleGroup.Flex & StyleGroup.Align & StyleGroup.Direction & StyleGroup.Gap & StyleGroup.Text & StyleGroup.Grid;
/** The roles a `<button>` takes; reference/elements.md says where the others go. */
export type ButtonRole = "gridcell" | "option" | "menuitem" | "tab" | "radio" | "treeitem" | "menuitemcheckbox" | "menuitemradio" | "combobox";
export type ButtonProps = Omit<Props, "style" | "children" | "role"> & { role?: ButtonRole; style?: ButtonStyle; children?: Child;
  /** A `treeitem`'s depth, 1..32; Left and Right walk between levels. */ "aria-level"?: number;
  /** With `role="combobox"`: the ids (1..8, space-separated) of the list it opens. Assistive tech and automation read them; an id that names no element is left out. */ "aria-controls"?: string;
  /** With `role="combobox"`: what is chosen, spoken after the name. */ "aria-valuetext"?: string;
  /** Checked state of a `radio`, `menuitemcheckbox` or `menuitemradio` item; `"mixed"` on a `menuitemcheckbox` that is partly checked. */ "aria-checked"?: boolean | "mixed";
  /** Called for a long press or a secondary click, with where it happened. */ onContextMenu?: (event: ContextMenuEvent) => void;
  /** Reports a single pointer over the element: down, move, up, enter, leave and cancel. */ onPointer?: (event: PointerEvent) => void;
  /** The axis `onPointer` keeps; the other is left to the page's scroll. */ pointerAxis?: "x" | "y" | "both";
  /** Called when it is pressed: a tap, a click, Enter or Space, or a screen reader's activation. `event.timeStamp` is when the press arrived, in epoch ms. */ onClick?: (event: PressEvent) => void;
  /** Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. */ disabled?: boolean;
  /** The accessible name, spoken in place of its text. */ "aria-label"?: string;
  /** The id of a popover a press opens or closes. */ popoverTarget?: string;
  /** The id of a `popover="hint"` shown while the pointer rests on it or it has focus. */ interestFor?: string;
  /** The current item of a set: `"page"` in navigation, `"step"` in a progress of steps, `true` otherwise; `false` or absent for the rest. */ "aria-current"?: "page" | "step" | boolean;
  /** On a menu item: run `onClick` and leave the menu open. */ keepOpen?: boolean;
  /** A press leaves the focused editor editing: its caret, selection and keyboard stay, as a format bar above the keyboard needs. */ keepsFocus?: boolean;
  /** Called when a mouse, stylus or trackpad pointer enters it. */ onPointerEnter?: () => void;
  /** Called when that pointer leaves it. */ onPointerLeave?: () => void;
} & ({ 
  /** Lets the finger move it and its subtree along one axis, between its `dragSnapPoints`. */ drag: "x" | "y"; 
  /** Up to 8 offsets, in pixels or percents of its size, a drag comes to rest at. */ dragSnapPoints?: readonly (number | `${number}%`)[]; 
  /** A number `useLocal` cell holding the index of the snap point it rests on. */ bind?: Local<number>; 
  /** The index of the snap point it rests on, with `onSnapChange`; not with `bind`. */ snapIndex?: number; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange?: (index: number) => void; 
  /** Called when a drag starts. */ onDragStart?: () => void; 
  /** Called when a drag is released, with the snap index it heads for and its velocity. */ onDragEnd?: (info: { index: number; velocity: number }) => void; 
  /** Ids of buttons (space-separated) a screen reader offers as this element's actions, as for a swipe row. */ "aria-actions"?: string; } | { 
  /** Lets the finger move it and its subtree along one axis, between its `dragSnapPoints`. */ drag?: undefined; 
  /** Up to 8 offsets, in pixels or percents of its size, a drag comes to rest at. */ dragSnapPoints?: never; 
  /** A number `useLocal` cell holding the index of the snap point it rests on. */ bind?: never; 
  /** The index of the snap point it rests on, with `onSnapChange`; not with `bind`. */ snapIndex?: never; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange?: never; 
  /** Called when a drag starts. */ onDragStart?: never; 
  /** Called when a drag is released, with the snap index it heads for and its velocity. */ onDragEnd?: never; 
  /** Ids of buttons (space-separated) a screen reader offers as this element's actions, as for a swipe row. */ "aria-actions"?: never; }) & ({ 
  /** The index of the snap point it rests on, with `onSnapChange`; not with `bind`. */ snapIndex: number; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange: (index: number) => void; 
  /** A number `useLocal` cell holding the index of the snap point it rests on. */ bind?: never; } | { 
  /** A number `useLocal` cell holding the index of the snap point it rests on. */ bind: Local<number>;  
  /** The index of the snap point it rests on, with `onSnapChange`; not with `bind`. */ snapIndex?: never; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange?: never; } | { 
  /** A number `useLocal` cell holding the index of the snap point it rests on. */ bind?: never; 
  /** The index of the snap point it rests on, with `onSnapChange`; not with `bind`. */ snapIndex?: never; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange?: never; });
export type ScrollStyle = StyleGroup.Box & StyleGroup.Padding & StyleGroup.Flex & StyleGroup.Align & StyleGroup.Gap & StyleGroup.Text & StyleGroup.Scroll & StyleGroup.Overscroll;
/** The roles a `<scroll>` takes; reference/elements.md says where the others go. */
export type ScrollRole = "list" | "grid" | "listbox" | "menu" | "group" | "tree";
export type ScrollProps = Omit<Props, "style" | "children" | "role"> & { role?: ScrollRole; style?: ScrollStyle; children?: Child;
  /** On a direct child of `<page>`: extend under the safe-area insets to the screen edge, on every side or the named ones. */ bleed?: BleedEdge;
  /** Called with its laid-out size after each change, one render behind. */ onLayout?: (box: BoxSize) => void;
  /** A `useBox` handle this element fills with its laid-out size. */ box?: Box;
  /** Called when the scroll reaches its start, as for loading older items. */ onReachStart?: () => void;
  /** Called when the scroll reaches its end, as for loading more items. */ onReachEnd?: () => void;
  /** Called when the scroll reaches or leaves either edge, and on mount with the edges it starts at. */ onScrollEdge?: (event: ScrollEdgeEvent) => void;
  /** Called with the children on screen (an element with an `id`, or a `List` row by its key) when that set, a child's ratio by a quarter step, or the current child changes, and once with exact values when the scroll comes to rest. */ onVisibleChange?: (event: VisibleChangeEvent) => void;
} & ({ 
  /** The index of the snap point a `scrollSnapType` scroll rests on, with `onSnapChange`; not with `bind`. */ snapIndex: number; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange: (index: number) => void; 
  /** A number `useLocal` cell holding the index of the snap point a `scrollSnapType` scroll rests on. */ bind?: never; } | { 
  /** A number `useLocal` cell holding the index of the snap point a `scrollSnapType` scroll rests on. */ bind: Local<number>;  
  /** The index of the snap point a `scrollSnapType` scroll rests on, with `onSnapChange`; not with `bind`. */ snapIndex?: never; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange?: never; } | { 
  /** A number `useLocal` cell holding the index of the snap point a `scrollSnapType` scroll rests on. */ bind?: never; 
  /** The index of the snap point a `scrollSnapType` scroll rests on, with `onSnapChange`; not with `bind`. */ snapIndex?: never; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange?: never; });
export type ListStyle = StyleGroup.Box & StyleGroup.Padding & StyleGroup.Gap & StyleGroup.Text & StyleGroup.Overscroll;
/** The roles a `<list>` takes; reference/elements.md says where the others go. */
export type ListRole = "list" | "grid" | "listbox" | "menu" | "group" | "tree";
export type ListProps = Omit<Props, "style" | "children" | "role"> & { role?: ListRole; style?: ListStyle; children?: never;
  /** On a direct child of `<page>`: extend under the safe-area insets to the screen edge, on every side or the named ones. */ bleed?: BleedEdge;
  /** How many rows the list has. */ count: number;
  /** Renders the row at an index; only rows near the screen are rendered. */ row: (index: number) => JSX.Element;
  /** The height of the row at an index, when rows differ; rows are measured otherwise. */ height?: (index: number) => number;
  /** `"end"` starts at the last row; `{ index, align, seq }` scrolls there, again whenever `seq` changes. */ anchor?: "end" | { index: number; align: "start" | "end"; seq?: number; duration?: number };
  /** A stable key per row, so state and position follow an item when rows move. */ itemKeys?: readonly string[];
  /** Called when the scroll reaches its start, as for loading older items. */ onReachStart?: () => void;
  /** Called when the scroll reaches its end, as for loading more items. */ onReachEnd?: () => void;
  /** Called when the scroll reaches or leaves either edge, and on mount with the edges it starts at. */ onScrollEdge?: (event: ScrollEdgeEvent) => void;
  /** Called with the children on screen (an element with an `id`, or a `List` row by its key) when that set, a child's ratio by a quarter step, or the current child changes, and once with exact values when the scroll comes to rest. */ onVisibleChange?: (event: VisibleChangeEvent) => void;
  /** The accessible name, spoken in place of its text. */ "aria-label"?: string;
};
export interface DataListProps<T> extends Omit<ListProps, "count" | "row" | "height" | "itemKeys"> {
  /** The items, one row each; only rows near the screen are rendered. */ items: readonly T[];
  /** A stable, short key for an item, so state and position follow it when items move. */ getKey: (item: T, index: number) => string;
  /** Renders the row for an item. */ renderItem: (item: T, index: number) => JSX.Element;
  /** The height of an item's row, when rows differ; rows are measured otherwise. */ getItemHeight?: (item: T, index: number) => number;
}
export type ImageStyle = StyleGroup.Box & StyleGroup.Media & StyleGroup.ImagePlacement;
/** The roles a `<image>` takes; reference/elements.md says where the others go. */
export type ImageRole = never;
export type ImageProps = Omit<Props, "style" | "children" | "role"> & { role?: ImageRole; style?: ImageStyle; children?: never;
  /** On a direct child of `<page>`: extend under the safe-area insets to the screen edge, on every side or the named ones. */ bleed?: BleedEdge;
  /** The picture: a package path, an `lx://` file, or a URL on an allowed host; PNG or JPEG. */ src: string;
  /** A picture shown until `src` has loaded, and if it fails. */ placeholder?: string;
  /** What the image shows, spoken by a screen reader; empty for a decorative image. */ alt?: string;
  /** Change it to load the same `src` again after an error. */ retryKey?: string;
  /** Called once per `src` and `retryKey`, when the picture has loaded. */ onLoad?: () => void;
  /** Called once per `src` and `retryKey`, when the picture fails to load. */ onError?: () => void;
};
export type InputStyle = StyleGroup.Box & StyleGroup.Padding & StyleGroup.Text & StyleGroup.Editor & StyleGroup.Ellipsis;
/** The roles a `<input>` takes; reference/elements.md says where the others go. */
export type InputRole = "combobox" | "spinbutton";
export type InputProps = Omit<Props, "style" | "children" | "role"> & { role?: InputRole; style?: InputStyle; children?: never;
  /** The value of a `progressbar`, a `spinbutton` or an `onPointer` element, between `aria-valuemin` and `aria-valuemax`; leave it out while a progressbar's amount is unknown. On an `onPointer` element it is the value arrows and a screen reader step. */ "aria-valuenow"?: number;
  /** The lowest value; 0 unless set. */ "aria-valuemin"?: number;
  /** The highest value; 100 unless set. */ "aria-valuemax"?: number;
  /** Spoken in place of the number, such as "1:20 of 3:05". */ "aria-valuetext"?: string;
  /** The id of the option the editor's keys point at while focus stays in the text; a screen reader hears that option as it changes. */ "aria-activedescendant"?: string;
  /** With `role="combobox"`: the ids (1..8, space-separated) of the list it opens. Assistive tech and automation read them; an id that names no element is left out. */ "aria-controls"?: string;
  /** With `role="combobox"`: `"list"` when typing filters the list it opens. */ "aria-autocomplete"?: "list" | "none";
  /** Keys the editor handles before typing: a map from key name to a handler returning `true` when it took the key. */ onKeyDown?: EditorKeys;
  /** The text can be selected and copied but not changed. */ readOnly?: boolean;
  /** Called when the editor gains focus. */ onFocus?: () => void;
  /** Called when the editor loses focus. */ onBlur?: () => void;
  /** Called at most once a frame when the caret moves, the selection changes or where it shows changes: `start` and `end` are UTF-16 offsets, as `value.slice(start, end)` reads them; `caret` and `rects` are in page points. To place a menu at the caret, anchor a popover to `<id>:caret` instead. Needs `bind` for `setSelection` and `replaceRange` on the cell. */ onSelectionChange?: (selection: EditorSelection) => void;
  /** Called when the user pastes, before anything is inserted: `text` raw, `html` as the source app wrote it (unsanitised; absent for a plain paste or a secure editor). Call `event.preventDefault()` synchronously to insert your own result, usually with `replaceRange(selection.start, selection.end, …)`; otherwise the plain text is inserted, a single-line editor taking newlines as spaces. */ onPaste?: (event: PasteEvent) => void;
  /** The accessible name, spoken in place of its text. */ "aria-label"?: string;
  /** Shown while the value is empty. */ placeholder?: string;
  /** The kind of value: picks the keyboard, masking, and the date or time picker. */ type?: "text" | "password" | "email" | "number" | "tel" | "search" | "date" | "time";
  /** The on-screen keyboard to show, without changing what is accepted. */ inputMode?: "text" | "numeric" | "decimal" | "tel" | "email";
  /** Focus the editor when it first appears. */ autoFocus?: boolean;
  /** Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. */ disabled?: boolean;
  /** Called with the text for the keyboard's action key, or Enter on a single-line editor. */ onSubmit?: (value: string) => void;
  /** The most characters it holds; typing stops there. */ maxLength?: number;
  /** What the value is, so the system can offer saved or received values, such as a one-time code. */ autoComplete?: AutoComplete;
  /** The label of the keyboard's action key. */ enterKeyHint?: "enter" | "done" | "go" | "next" | "previous" | "search" | "send";
  /** Which letters the keyboard capitalises on its own. */ autoCapitalize?: "none" | "sentences" | "words" | "characters" | "off" | "on";
  /** Whether misspellings are marked and corrected. */ spellCheck?: boolean;
  /** The lowest value: a number for `type="number"`, a date or time string for a picker. */ min?: number | string;
  /** The highest value: a number for `type="number"`, a date or time string for a picker. */ max?: number | string;
  /** The interval a number snaps to. */ step?: number;
} & ({ bind: Local<string>; value?: never; onInput?: never } | { bind?: never; value?: string; onInput?: (value: string) => void });
export type TextareaStyle = StyleGroup.Box & StyleGroup.Padding & StyleGroup.Text & StyleGroup.Editor;
/** The roles a `<textarea>` takes; reference/elements.md says where the others go. */
export type TextareaRole = never;
export type TextareaProps = Omit<Props, "style" | "children" | "role"> & { role?: TextareaRole; style?: TextareaStyle; children?: never;
  /** Keys the editor handles before typing: a map from key name to a handler returning `true` when it took the key. */ onKeyDown?: EditorKeys;
  /** The text can be selected and copied but not changed. */ readOnly?: boolean;
  /** Called when the editor gains focus. */ onFocus?: () => void;
  /** Called when the editor loses focus. */ onBlur?: () => void;
  /** Called at most once a frame when the caret moves, the selection changes or where it shows changes: `start` and `end` are UTF-16 offsets, as `value.slice(start, end)` reads them; `caret` and `rects` are in page points. To place a menu at the caret, anchor a popover to `<id>:caret` instead. Needs `bind` for `setSelection` and `replaceRange` on the cell. */ onSelectionChange?: (selection: EditorSelection) => void;
  /** Called when the user pastes, before anything is inserted: `text` raw, `html` as the source app wrote it (unsanitised; absent for a plain paste or a secure editor). Call `event.preventDefault()` synchronously to insert your own result, usually with `replaceRange(selection.start, selection.end, …)`; otherwise the plain text is inserted, a single-line editor taking newlines as spaces. */ onPaste?: (event: PasteEvent) => void;
  /** Lines it shows when empty; it grows with its text up to `maxRows`. */ rows?: number;
  /** Lines it grows to before it scrolls. */ maxRows?: number;
  /** The accessible name, spoken in place of its text. */ "aria-label"?: string;
  /** Shown while the value is empty. */ placeholder?: string;
  /** The on-screen keyboard to show, without changing what is accepted. */ inputMode?: "text" | "numeric" | "decimal" | "tel" | "email";
  /** Focus the editor when it first appears. */ autoFocus?: boolean;
  /** Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. */ disabled?: boolean;
  /** Called with the text for the keyboard's action key, or Enter on a single-line editor. */ onSubmit?: (value: string) => void;
  /** The most characters it holds; typing stops there. */ maxLength?: number;
  /** What the value is, so the system can offer saved or received values, such as a one-time code. */ autoComplete?: AutoComplete;
  /** The label of the keyboard's action key. */ enterKeyHint?: "enter" | "done" | "go" | "next" | "previous" | "search" | "send";
  /** Which letters the keyboard capitalises on its own. */ autoCapitalize?: "none" | "sentences" | "words" | "characters" | "off" | "on";
  /** Whether misspellings are marked and corrected. */ spellCheck?: boolean;
} & ({ bind: Local<string>; value?: never; onInput?: never } | { bind?: never; value?: string; onInput?: (value: string) => void });
export type CheckboxStyle = StyleGroup.Box & StyleGroup.Padding & StyleGroup.Flex & StyleGroup.Align & StyleGroup.Gap & StyleGroup.Text;
/** The roles a `<checkbox>` takes; reference/elements.md says where the others go. */
export type CheckboxRole = never;
export type CheckboxProps = Omit<Props, "style" | "children" | "role"> & { role?: CheckboxRole; style?: CheckboxStyle; children?: Child;
  /** Shows a mixed state, as for a parent of partly checked items; a press clears it. */ indeterminate?: boolean;
  /** Where an unbound control starts; it applies once. */ defaultChecked?: boolean;
  /** The accessible name, spoken in place of its text. */ "aria-label"?: string;
  /** Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. */ disabled?: boolean;
} & ({ bind: Local<boolean>; value?: never; onChange?: never } | { bind?: never; value?: boolean; onChange?: (value: ReadonlyData<boolean>) => void });
export type SwitchStyle = StyleGroup.Box & StyleGroup.Padding & StyleGroup.Text;
/** The roles a `<switch>` takes; reference/elements.md says where the others go. */
export type SwitchRole = never;
export type SwitchProps = Omit<Props, "style" | "children" | "role"> & { role?: SwitchRole; style?: SwitchStyle; children?: Child;
  /** Where an unbound control starts; it applies once. */ defaultChecked?: boolean;
  /** The accessible name, spoken in place of its text. */ "aria-label"?: string;
  /** Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. */ disabled?: boolean;
} & ({ bind: Local<boolean>; value?: never; onChange?: never } | { bind?: never; value?: boolean; onChange?: (value: ReadonlyData<boolean>) => void });
export type RadioStyle = StyleGroup.Box & StyleGroup.Padding & StyleGroup.Gap & StyleGroup.Text;
/** The roles a `<radio>` takes; reference/elements.md says where the others go. */
export type RadioRole = never;
export type RadioProps = Omit<Props, "style" | "children" | "role"> & { role?: RadioRole; style?: RadioStyle; children?: never;
  /** The choices, each a value and a label. */ options: readonly ChoiceOption[];
  /** The accessible name, spoken in place of its text. */ "aria-label"?: string;
  /** Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. */ disabled?: boolean;
} & ({ bind: Local<string>; value?: never; onChange?: never } | { bind?: never; value?: string; onChange?: (value: string) => void });
export type SliderStyle = StyleGroup.Box & StyleGroup.Padding & StyleGroup.Accent;
/** The roles a `<slider>` takes; reference/elements.md says where the others go. */
export type SliderRole = never;
export type SliderProps = Omit<Props, "style" | "children" | "role"> & { role?: SliderRole; style?: SliderStyle; children?: never;
  /** The lowest value it accepts. */ min?: number;
  /** The highest value it accepts. */ max?: number;
  /** The interval values snap to. */ step?: number;
  /** Where an unbound slider starts, snapped to `step`; it applies once. */ defaultValue?: number;
  /** A second range drawn behind the value, such as buffered media. */ secondaryValue?: number;
  /** The accessible name, spoken in place of its text. */ "aria-label": string;
  /** Spoken in place of the number, such as "1:20 of 3:05". */ "aria-valuetext"?: string;
  /** Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. */ disabled?: boolean;
  /** Called with `true` when a finger or pointer takes the thumb and `false` when it lets go. */ onPressChange?: (pressed: boolean) => void;
} & ({ bind: Local<number>; value?: never; onChange?: never; onChangeEnd?: (value: ReadonlyData<number>) => void; values?: never; thumbLabels?: never } | { bind?: never; value?: number; onChange?: (value: ReadonlyData<number>) => void; onChangeEnd?: (value: ReadonlyData<number>) => void; values?: never; thumbLabels?: never } | { bind?: never; value?: never; values: readonly [number, number]; thumbLabels: readonly [string, string]; onChange?: (values: readonly [number, number]) => void; onChangeEnd?: (values: readonly [number, number]) => void });
export type DialogStyle = StyleGroup.Box & StyleGroup.Padding & StyleGroup.Flex & StyleGroup.Align & StyleGroup.Direction & StyleGroup.Gap & StyleGroup.Text & StyleGroup.Grid;
/** The roles a `<dialog>` takes; reference/elements.md says where the others go. */
export type DialogRole = "alertdialog";
export type DialogProps = Omit<Props, "style" | "children" | "role"> & { role?: DialogRole; style?: DialogStyle; children?: Child;
  /** Always true for a `<dialog>`; `false` is refused. */ "aria-modal"?: boolean;
  /** Whether the dialog is shown; with `onClose` to follow a close request. */ open?: boolean;
  /** Called for a request to close: Escape, back, a backdrop tap or a sheet dragged off; it closes when you set `open` to false. */ onClose?: () => void;
  /** The close requests it takes: `closerequest` (default) Escape and back, `any` also a backdrop tap, `none` none. */ closedby?: "any" | "closerequest" | "none";
  /** The accessible name, spoken in place of its text. */ "aria-label": string;
} & ({ 
  /** Lets the finger move it and its subtree along one axis, between its `dragSnapPoints`. */ drag: "x" | "y"; 
  /** Up to 8 offsets, in pixels or percents of its size, a drag comes to rest at. */ dragSnapPoints?: readonly (number | `${number}%`)[]; 
  /** A number `useLocal` cell holding the index of the snap point it rests on. */ bind?: Local<number>; 
  /** The index of the snap point it rests on, with `onSnapChange`; not with `bind`. */ snapIndex?: number; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange?: (index: number) => void; 
  /** Called when a drag starts. */ onDragStart?: () => void; 
  /** Called when a drag is released, with the snap index it heads for and its velocity. */ onDragEnd?: (info: { index: number; velocity: number }) => void; } | { 
  /** Lets the finger move it and its subtree along one axis, between its `dragSnapPoints`. */ drag?: undefined; 
  /** Up to 8 offsets, in pixels or percents of its size, a drag comes to rest at. */ dragSnapPoints?: never; 
  /** A number `useLocal` cell holding the index of the snap point it rests on. */ bind?: never; 
  /** The index of the snap point it rests on, with `onSnapChange`; not with `bind`. */ snapIndex?: never; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange?: never; 
  /** Called when a drag starts. */ onDragStart?: never; 
  /** Called when a drag is released, with the snap index it heads for and its velocity. */ onDragEnd?: never; }) & ({ 
  /** The index of the snap point it rests on, with `onSnapChange`; not with `bind`. */ snapIndex: number; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange: (index: number) => void; 
  /** A number `useLocal` cell holding the index of the snap point it rests on. */ bind?: never; } | { 
  /** A number `useLocal` cell holding the index of the snap point it rests on. */ bind: Local<number>;  
  /** The index of the snap point it rests on, with `onSnapChange`; not with `bind`. */ snapIndex?: never; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange?: never; } | { 
  /** A number `useLocal` cell holding the index of the snap point it rests on. */ bind?: never; 
  /** The index of the snap point it rests on, with `onSnapChange`; not with `bind`. */ snapIndex?: never; 
  /** Called with the snap point index it came to rest at after a drag or swipe. */ onSnapChange?: never; });
export type PlotStyle = StyleGroup.Box;
/** The roles a `<plot>` takes; reference/elements.md says where the others go. */
export type PlotRole = never;
export type PlotProps = Omit<Props, "style" | "children" | "role"> & { role?: PlotRole; style?: PlotStyle; children?: never;
  /** Called with its laid-out size after each change, one render behind. */ onLayout?: (box: BoxSize) => void;
  /** A `useBox` handle this element fills with its laid-out size. */ box?: Box;
  /** The chart: bars, a line, or a pie. */ kind: "bar" | "line" | "pie";
  /** The data, one number per bar, point or slice. */ values: readonly number[];
  /** Chart colour roles, 1..16, cycled over bars and slices; a line uses the first. */ colors: readonly ColorSeries[];
  /** The accessible name, spoken in place of its text. */ "aria-label": string;
  /** Draw a line chart as a smooth curve. */ smooth?: boolean;
  /** Fill under a line chart. */ area?: boolean;
  /** The highlighted bar, point or slice; pass back what `onSelect` reports. */ selectedIndex?: number;
  /** The lowest value the axis shows; it must contain zero and every value. */ domainMin?: number;
  /** The highest value the axis shows; it must contain zero and every value. */ domainMax?: number;
  /** Called with the index of the bar, point or slice a press picks. */ onSelect?: (index: number) => void;
  /** The pie's centre and radii, as fractions of the box. */ pieLayout?: PieLayout;
  /** How much larger the selected point's marker is drawn. */ markerEmphasisScale?: number;
};
export type VectorStyle = StyleGroup.Box & StyleGroup.Padding & StyleGroup.Accent & StyleGroup.Media;
/** The roles a `<vector>` takes; reference/elements.md says where the others go. */
export type VectorRole = never;
export type VectorProps = Omit<Props, "style" | "children" | "role"> & { role?: VectorRole; style?: VectorStyle; children?: never;
  /** On a direct child of `<page>`: extend under the safe-area insets to the screen edge, on every side or the named ones. */ bleed?: BleedEdge;
  /** Reports a single pointer over the element: down, move, up, enter, leave and cancel. */ onPointer?: (event: PointerEvent) => void;
  /** The axis `onPointer` keeps; the other is left to the page's scroll. */ pointerAxis?: "x" | "y" | "both";
  /** Called with its laid-out size after each change, one render behind. */ onLayout?: (box: BoxSize) => void;
  /** A `useBox` handle this element fills with its laid-out size. */ box?: Box;
  /** The shapes to draw: SVG path data with a fill and a stroke each. */ paths: ReadonlyArray<VectorPath>;
  /** Up to 8 named gradients the paths use as `fill: 'url(#id)'`. */ gradients?: Readonly<Record<string, VectorGradient>>;
  /** The drawing's coordinate box, `[x, y, width, height]`, scaled to fit the element. */ viewBox?: readonly [number, number, number, number];
};
export type SpanStyle = StyleGroup.Run;
export type SpanProps = Omit<Props, "style" | "children" | "id" | "role" | "aria-live" | "aria-pressed" | "aria-selected" | "aria-expanded" | "aria-busy" | "aria-invalid" | "aria-required" | "aria-describedby"> & { style?: SpanStyle; children?: RunChild;
};
export type LinkStyle = StyleGroup.Run & StyleGroup.Box & StyleGroup.Padding & StyleGroup.Flex & StyleGroup.Align & StyleGroup.Direction & StyleGroup.Gap & StyleGroup.Text & StyleGroup.Grid;
/** The roles a `<link>` takes; reference/elements.md says where the others go. */
export type LinkRole = "menuitem";
export type LinkProps = Omit<Props, "style" | "children" | "role"> & { role?: LinkRole; style?: LinkStyle; children?: Child;
  /** Called for a long press or a secondary click, with where it happened. */ onContextMenu?: (event: ContextMenuEvent) => void;
  /** Called when it is pressed: a tap, a click, Enter or Space, or a screen reader's activation. `event.timeStamp` is when the press arrived, in epoch ms. */ onClick?: (event: PressEvent) => void;
  /** The page a press navigates to, with its params. */ to?: LinkTarget;
  /** The accessible name, spoken in place of its text. */ "aria-label"?: string;
  /** Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. */ disabled?: boolean;
  /** The id of a popover a press opens or closes. */ popoverTarget?: string;
  /** The id of a `popover="hint"` shown while the pointer rests on it or it has focus. */ interestFor?: string;
  /** The current item of a set: `"page"` in navigation, `"step"` in a progress of steps, `true` otherwise; `false` or absent for the rest. */ "aria-current"?: "page" | "step" | boolean;
};
export type SelectStyle = StyleGroup.Box & StyleGroup.Padding & StyleGroup.Text & StyleGroup.TextBlock & StyleGroup.Editor;
/** The roles a `<select>` takes; reference/elements.md says where the others go. */
export type SelectRole = never;
export type SelectProps = Omit<Props, "style" | "children" | "role"> & { role?: SelectRole; style?: SelectStyle; children?: never;
  /** The choices, each a value and a label. */ options: readonly ChoiceOption[];
  /** The option an unbound select starts on; it applies once. */ defaultValue?: string;
  /** Shown while the value is empty. */ placeholder?: string;
  /** The accessible name, spoken in place of its text. */ "aria-label"?: string;
  /** Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. */ disabled?: boolean;
} & ({ bind: Local<string>; value?: never; onChange?: never } | { bind?: never; value?: string; onChange?: (value: string) => void });
export type VideoStyle = StyleGroup.Box & StyleGroup.Media & StyleGroup.ImagePlacement;
/** The roles a `<video>` takes; reference/elements.md says where the others go. */
export type VideoRole = never;
export type VideoProps = Omit<Props, "style" | "children" | "role"> & { role?: VideoRole; style?: VideoStyle; children?: never;
  /** On a direct child of `<page>`: extend under the safe-area insets to the screen edge, on every side or the named ones. */ bleed?: BleedEdge;
  /** A `useVideo` player: control it from code, or show the same player in another `<video>`. */ player?: Video;
  /** An image shown until the first frame. */ poster?: string;
  /** Start playing once it can. */ autoPlay?: boolean;
  /** Pause or play; applies on mount and when it changes. */ paused?: boolean;
  /** Mute or unmute; applies on mount and when it changes. */ muted?: boolean;
  /** Start over at the end. */ loop?: boolean;
  /** Playback speed, 1 normal. */ playbackRate?: number;
  /** Volume, 0..1. */ volume?: number;
  /** The accessible name, spoken in place of its text. */ "aria-label"?: string;
  /** Called when the duration and size are known. */ onLoadedMetadata?: (event: VideoMetadata & TimeStamped) => void;
  /** Called when playback is asked to start. */ onPlay?: (event: TimeStamped) => void;
  /** Called when frames start moving, after a start or a stall. */ onPlaying?: (event: TimeStamped) => void;
  /** Called when playback pauses, with why. */ onPause?: (event: VideoPause & TimeStamped) => void;
  /** Called when playback stalls for data. */ onWaiting?: (event: TimeStamped) => void;
  /** Called when a seek has finished, with the new time. */ onSeeked?: (event: { readonly currentTime: number } & TimeStamped) => void;
  /** Called as the playback time moves, a few times a second. */ onTimeUpdate?: (event: { readonly currentTime: number } & TimeStamped) => void;
  /** Called when playback reaches the end. */ onEnded?: (event: TimeStamped) => void;
  /** Called when the video fails to load or play. */ onError?: (error: VideoError & TimeStamped) => void;
  /** Called when the playback speed changes. */ onRateChange?: (event: { readonly playbackRate: number } & TimeStamped) => void;
  /** Called when the volume or mute changes. */ onVolumeChange?: (event: { readonly volume: number; readonly muted: boolean } & TimeStamped) => void;
  /** Called when a new quality is on screen. */ onQualityChange?: (event: VideoQualityChange & TimeStamped) => void;
  /** Called when it enters or leaves full screen. */ onFullscreenChange?: (event: { readonly fullscreen: boolean }) => void;
} & ({ 
  /** The same video at several qualities; not with `src`. */ sources: readonly VideoSource[];  
  /** The video to play; not with `sources`. */ src?: never; } | { 
  /** The video to play; not with `sources`. */ src: string;  
  /** The same video at several qualities; not with `src`. */ sources?: never; } | { 
  /** The video to play; not with `sources`. */ src?: never; 
  /** The same video at several qualities; not with `src`. */ sources?: never; 
  /** A `useVideo` player: control it from code, or show the same player in another `<video>`. */ player: Video; });
export type WebviewStyle = StyleGroup.Box;
/** The roles a `<webview>` takes; reference/elements.md says where the others go. */
export type WebviewRole = never;
export type WebviewProps = Omit<Props, "style" | "children" | "role"> & { role?: WebviewRole; style?: WebviewStyle; children?: never;
  /** On a direct child of `<page>`: extend under the safe-area insets to the screen edge, on every side or the named ones. */ bleed?: BleedEdge;
  /** The page to load. */ src?: string;
  /** A `useWebView` document: pass the same one to each `<webview>` that shows it, keeping one live page. */ web?: WebView;
  /** The accessible name, spoken in place of its text. */ "aria-label": string;
  /** The web page handles its own drags (a map, a canvas); a drag starting on it never passes to the page. */ handlesDrags?: boolean;
  /** Called when it enters or leaves full screen. */ onFullscreenChange?: (event: { readonly fullscreen: boolean }) => void;
  /** Called when a page has loaded, with its address and title. */ onLoad?: (event: WebViewLoad) => void;
  /** Called with what the web page posts through `window.lingxia.postMessage`. */ onMessage?: (message: WebViewMessage) => void;
  /** Called with a navigation the host cancelled: outside trustedDomains, or not https. */ onBlocked?: (event: WebViewBlocked) => void;
  /** Called when a page fails to load. */ onError?: (error: WebViewError) => void;
};

declare global {
  namespace JSX {
    type ElementType = keyof IntrinsicElements | ((props: any) => ComponentResult);
    interface Element { readonly __ui: unique symbol }
    interface IntrinsicAttributes { key?: string }
    interface ElementChildrenAttribute { children: {} }
    interface IntrinsicElements {
      "view": ViewProps;
      "page": PageProps;
      "text": TextProps;
      "button": ButtonProps;
      "scroll": ScrollProps;
      "image": ImageProps;
      "input": InputProps;
      "textarea": TextareaProps;
      "checkbox": CheckboxProps;
      "switch": SwitchProps;
      "radio": RadioProps;
      "slider": SliderProps;
      "dialog": DialogProps;
      "plot": PlotProps;
      "vector": VectorProps;
      "span": SpanProps;
      "link": LinkProps;
      "select": SelectProps;
      "video": VideoProps;
      "webview": WebviewProps;
    }
  }
}
