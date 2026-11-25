// Hook, component and data types of the authoring API. The generator appends
// element props and the CSS-named Style from schema.json.

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
/** `code` is the `E_` string; `reason` names the business case (camelCase) and `detail` adds what the message leaves out. */
export interface ResourceError {
  readonly code: string; readonly message: string;
  readonly reason?: string; readonly detail?: string; readonly data?: unknown;
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
/** The size of the element that takes this handle as `box`; `null` until its first layout, then one render behind a change. */
export interface Box { readonly value: BoxSize | null }
export declare function useBox(name: string): Box;
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
