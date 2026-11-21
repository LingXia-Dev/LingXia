// Generated from contracts/view/schema.json; do not edit.

export type Child = JSX.Element | string | number | boolean | null | undefined | readonly Child[];
export type TextChild = string | number | boolean | null | undefined | readonly TextChild[];
/** Text content of `<text>` and `<span>`: strings, `<span>` and `<link>` runs. */
export type RunChild = TextChild | JSX.Element | readonly RunChild[];
export type ComponentResult = JSX.Element | null | undefined | boolean;
export type Component<P = {}> = (props: P) => ComponentResult;
export type ComponentProps<C> = C extends (props: infer P) => unknown ? P : never;
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
  headingLevel?: 1 | 2 | 3 | 4 | 5 | 6;
  /** A progressbar's value between `aria-valuemin` (0) and `aria-valuemax` (100); leave it out while the amount is unknown. `aria-valuetext` replaces the spoken percentage. On an `onPointer` element it is the value arrows and a screen reader step. */
  "aria-valuenow"?: number; "aria-valuemin"?: number; "aria-valuemax"?: number; "aria-valuetext"?: string;
  /** On an `<input>` or `<textarea>`: the id of the option the editor's keys point at while focus stays in the text; a screen reader hears that option as it changes. */
  "aria-activedescendant"?: string;
  /** With `role="combobox"`: the ids (1..8, space-separated) of the list it opens, and `"list"` when typing filters that list. Assistive tech and automation read both; an id that names no element is left out. */
  "aria-controls"?: string; "aria-autocomplete"?: "list" | "none";
  /** Removes the node and its subtree from accessibility. */
  "aria-hidden"?: boolean;
  /** On a popover with `role="dialog"`: modal like a `<dialog>`, which always is. Tab stays inside, the rest of the page leaves accessibility, and the page shows over the host's bars. Without it the popover dialog is non-modal. */
  "aria-modal"?: boolean;
  /** Announce changes; `alert` is assertive and `status` polite unless set. */
  "aria-live"?: "off" | "polite" | "assertive";
  /** States assistive input announces; leave one out when the element has no such state. */
  "aria-pressed"?: boolean; "aria-selected"?: boolean; "aria-expanded"?: boolean; "aria-busy"?: boolean;
  /** A field's error and required states; the error text belongs in `aria-describedby`. */
  "aria-invalid"?: boolean; "aria-required"?: boolean;
  /** Checked state of a `radio`, `menuitemcheckbox` or `menuitemradio` item; `"mixed"` on a `menuitemcheckbox` that is partly checked. */
  "aria-checked"?: boolean | "mixed";
  /** A `treeitem`'s depth, 1..32; Left and Right walk between levels. */
  "aria-level"?: number;
  /** Ids of elements (1..8, space-separated) whose text is read after the name, as a hint or error. */
  "aria-describedby"?: string;
}
export type LeafProps = Omit<Props, "children"> & { children?: never };


export type ReadonlyData<T> = T extends object ? { readonly [K in keyof T]: ReadonlyData<T[K]> } : T;
/** A failed resource or action, as reported by the bridge. */
export interface ResourceError { readonly code: string; readonly message: string }

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

export type ViewActions<A> = { readonly [K in keyof A]: A[K] extends (...args: infer P) => Promise<infer R> ? (...args: P) => Promise<ReadonlyData<R>> : A[K] };
export interface View<A = Readonly<Record<string, (payload?: unknown) => Promise<unknown>>>, D = Record<string, unknown>> {
  readonly data: ReadonlyData<D>; readonly actions: ViewActions<A>; readonly channels: Channels; readonly env: Env;
}
/** The View of the page being rendered; components needing typed data take it as props. */
export declare function useView(): View;

declare const localCell: unique symbol;
/** JSON-shaped state; undefined object fields are omitted, including optional fields. */
export type LocalData<T> = T extends string | number | boolean | null ? T
  : T extends (...args: any[]) => any ? never
  : T extends readonly unknown[] ? { [K in keyof T]: LocalData<T[K]> }
  : T extends object ? { [K in keyof T]: T[K] extends undefined ? undefined : LocalData<T[K]> } : never;
export interface Local<T> { readonly [localCell]: true; readonly value: ReadonlyData<T>; set(value: ReadonlyData<LocalData<T>>): void;
  /** Focus the editor this cell is bound to (`bind`), on the next frame. */
  focus(): void }
/** Local state keyed by call order in the component, as React's `useState`. */
export declare function useLocal<T>(initial: T, ...invalid: [T] extends [LocalData<T>] ? [] : [never]): Local<T>;
/** Local state keyed by `name`: it survives reordered calls and hot edits. */
export declare function useLocal<T>(name: string, initial: T, ...invalid: [T] extends [LocalData<T>] ? [] : [never]): Local<T>;

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

export interface StreamResource<P extends unknown[], R> {
  readonly status: "idle" | "streaming" | "completed" | "canceled" | "failed";
  readonly error: ResourceError | null;
  readonly result: ReadonlyData<R> | null;
  start(...params: P): void;
  cancel(): void;
}
export declare function useStream<P extends [] | [unknown?], T, R>(name: string, action: (...params: P) => Stream<T, R>, options?: { onData?: (value: ReadonlyData<T>) => void }): StreamResource<P, R>;

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

export interface DataListProps<T> extends Omit<ListProps, "count" | "itemKeys" | "row" | "height"> {
  items: readonly T[];
  getKey: (item: T, index: number) => string;
  renderItem: (item: T, index: number) => JSX.Element;
  getItemHeight?: (item: T, index: number) => number;
}
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
export interface RadioOption { readonly value: string; readonly label: string; readonly disabled?: boolean; readonly "data-testid"?: string }
/** HTML autocomplete tokens; Android maps them to autofill hints. */
export type AutoComplete = "off" | "on" | "username" | "current-password" | "new-password" | "one-time-code" | "email" | "tel" | "name" | "given-name" | "family-name" | "street-address" | "postal-code" | "country" | "cc-number" | "cc-exp" | "cc-csc" | "bday";

/** The page's environment (`view.env`): read-only, and a change re-renders the page. */
export interface Env {
  readonly theme: "light" | "dark";
  /** The page surface in logical pixels. */
  readonly viewport: Readonly<{ width: number; height: number }>;
  /** System bars and host chrome over the surface, logical pixels. */
  readonly insets: Readonly<{ top: number; right: number; bottom: number; left: number }>;
  /** The system bars alone (status bar, cutout, home indicator), as CSS `env(safe-area-inset-*)`; a dialog covering host chrome pads by these, logical pixels. */
  readonly safeArea: Readonly<{ top: number; right: number; bottom: number; left: number }>;
  readonly keyboard: Readonly<{ height: number }>;
  /** The host's capsule (the ··· and close control) where it floats over the page, in page coordinates, logical pixels; `null` when the host shows none or it sits in a nav bar. The same box Web pages read as `--lx-page-chrome-capsule-*`. */
  readonly capsule: Readonly<{ x: number; y: number; width: number; height: number }> | null;
  /** `fine` pointers (mouse, trackpad) hover; `coarse` ones (touch) do not. */
  readonly pointer: "fine" | "coarse";
  readonly os: string;
  /** `phone` or `tablet` from the smallest screen width; other hosts may report their own. */
  readonly formFactor: string;
  /** `compact` below 600 logical pixels of page width, else `regular`. */
  readonly sizeClass: "compact" | "regular";
  readonly displayLanguage: string;
  /** The language the lxapp's text is in: `displayLanguage` narrowed to the `languages` its lxapp.json lists. Built-in words, direction and glyph forms follow it. */
  readonly contentLanguage: string;
  /** The system asks for reduced motion; `motion-reduce:`/`motion-safe:` follow it. */
  readonly reducedMotion: boolean;
  /** The system asks for increased contrast; the palette resolves its high-contrast variant. */
  readonly highContrast: boolean;
  /** The platform's animation duration multiplier (0..10, default 1); 0 also sets `reducedMotion`. */
  readonly animatorDurationScale: number;
  /** 12- or 24-hour clock: the user's system setting, else the custom of `displayLanguage`. */
  readonly hourCycle: "h12" | "h23";
  /** The first day of the week, 1 = Sunday to 7 = Saturday: the user's system setting, else the custom of `displayLanguage`. */
  readonly firstWeekday: number;
  /** Inline direction derived from `contentLanguage`. */
  readonly direction: "ltr" | "rtl";
  /** Whether the window can dock an `as: 'aside'` beside the main; false until the host reports it. */
  readonly canDock: boolean;
  /** `fallback: "driver"` when this device's GPU driver failed and pages render on the CPU. */
  readonly renderer?: Readonly<{ fallback: "driver" | null }>;
  /** How much the system's text size grows body text: 1 at the default size, 2 when it is twice as large, after the lxapp's `maxTextScale`. Sizes already follow it; read it only for a layout that must change shape, such as fewer columns. */
  readonly textScale: number;
}

/** CSS-named style; each element accepts the subset its `*Style` type lists. */
export interface Style {
  width?: number | `${number}%` | `${number}vw` | `${number}vh` | `${number}ch` | "auto" | "fit-content" | "max-content" | "min-content";
  height?: number | `${number}%` | `${number}vw` | `${number}vh` | "auto" | "fit-content" | "max-content" | "min-content";
  minWidth?: number | `${number}%` | `${number}vw` | `${number}vh` | `${number}ch` | "fit-content" | "max-content" | "min-content";
  minHeight?: number | `${number}%` | `${number}vw` | `${number}vh` | "fit-content" | "max-content" | "min-content";
  maxWidth?: number | `${number}%` | `${number}vw` | `${number}vh` | `${number}ch` | "fit-content" | "max-content" | "min-content";
  aspectRatio?: number | "auto";
  maxHeight?: number | `${number}%` | `${number}vw` | `${number}vh` | "fit-content" | "max-content" | "min-content";
  margin?: number | "auto";
  marginTop?: number | "auto";
  marginRight?: number | "auto";
  marginBottom?: number | "auto";
  marginLeft?: number | "auto";
  marginInlineStart?: number | "auto";
  marginInlineEnd?: number | "auto";
  position?: "relative" | "absolute" | "fixed" | "sticky";
  inset?: number | `${number}%`;
  top?: number | `${number}%`;
  right?: number | `${number}%`;
  bottom?: number | `${number}%`;
  left?: number | `${number}%`;
  insetInlineStart?: number | `${number}%`;
  insetInlineEnd?: number | `${number}%`;
  zIndex?: number;
  positionArea?: "bottom span-inline-end" | "bottom span-inline-start" | "bottom" | "top span-inline-end" | "top span-inline-start" | "top" | "inline-end span-block-end" | "inline-end span-block-start" | "inline-end" | "inline-start span-block-end" | "inline-start span-block-start" | "inline-start";
  flexGrow?: number;
  flexShrink?: number;
  flexBasis?: number | `${number}%` | "auto";
  alignSelf?: "flex-start" | "start" | "center" | "flex-end" | "end" | "stretch" | "baseline" | "safe center" | "safe flex-end" | "safe end";
  gridColumn?: number | string;
  gridColumnStart?: number | string;
  gridColumnEnd?: number | string;
  gridRow?: number | string;
  gridRowStart?: number | string;
  gridRowEnd?: number | string;
  justifySelf?: "start" | "center" | "end" | "stretch" | "baseline" | "safe center" | "safe end";
  order?: number;
  opacity?: number;
  overflow?: "visible" | "hidden";
  backgroundColor?: string | "currentColor";
  backgroundImage?: BackgroundGradient | "none";
  borderRadius?: number;
  borderTopLeftRadius?: number;
  borderTopRightRadius?: number;
  borderBottomRightRadius?: number;
  borderBottomLeftRadius?: number;
  borderStartStartRadius?: number;
  borderStartEndRadius?: number;
  borderEndStartRadius?: number;
  borderEndEndRadius?: number;
  borderWidth?: number;
  borderTopWidth?: number;
  borderRightWidth?: number;
  borderBottomWidth?: number;
  borderLeftWidth?: number;
  borderInlineStartWidth?: number;
  borderInlineEndWidth?: number;
  borderStyle?: "solid" | "dashed" | "dotted";
  borderColor?: string | "currentColor";
  boxShadow?: ReadonlyArray<BoxShadow> | "none";
  ring?: Ring | "none";
  backdropFilter?: string;
  translate?: number | `${number}%` | readonly [number | `${number}%`, number | `${number}%`];
  rotate?: number;
  scale?: number | readonly [number, number];
  transformOrigin?: readonly [number, number];
  filter?: string;
  padding?: number;
  paddingTop?: number;
  paddingRight?: number;
  paddingBottom?: number;
  paddingLeft?: number;
  paddingInlineStart?: number;
  paddingInlineEnd?: number;
  display?: "flex" | "grid" | "none" | "block" | "inline-flex" | "inline-block";
  touchTarget?: "auto" | "none";
  flexDirection?: "row" | "column" | "row-reverse" | "column-reverse";
  alignItems?: "flex-start" | "start" | "center" | "flex-end" | "end" | "stretch" | "baseline" | "safe center" | "safe flex-end" | "safe end";
  justifyContent?: "flex-start" | "start" | "center" | "flex-end" | "end" | "space-between" | "space-around" | "space-evenly" | "safe center" | "safe flex-end" | "safe end";
  alignContent?: "flex-start" | "start" | "center" | "flex-end" | "end" | "space-between" | "space-around" | "space-evenly" | "stretch" | "safe center" | "safe flex-end" | "safe end";
  flexWrap?: "nowrap" | "wrap" | "wrap-reverse";
  gap?: number;
  rowGap?: number;
  columnGap?: number;
  gridTemplateColumns?: number | string;
  gridTemplateRows?: number | string;
  gridAutoColumns?: string;
  gridAutoRows?: string;
  gridAutoFlow?: "row" | "column" | "row dense" | "column dense";
  justifyItems?: "start" | "center" | "end" | "stretch" | "baseline" | "safe center" | "safe end";
  color?: string;
  fontSize?: number;
  fontWeight?: number;
  fontFamily?: "sans-serif" | "monospace" | "serif" | (string & {});
  fontStyle?: "normal" | "italic";
  lineHeight?: number | `${number}px`;
  letterSpacing?: number | `${number}em`;
  fontVariantNumeric?: string;
  userSelect?: "none" | "text" | "all";
  textAlign?: "start" | "end" | "left" | "center" | "right" | "justify";
  textDecorationLine?: "none" | "line-through" | "underline";
  textDecorationStyle?: "solid" | "dashed" | "dotted";
  textDecorationColor?: string | "currentColor";
  textTransform?: "none" | "uppercase" | "lowercase" | "capitalize";
  backgroundClip?: "border-box" | "text";
  whiteSpace?: "normal" | "nowrap" | "pre" | "pre-wrap" | "pre-line";
  wordBreak?: "normal" | "break-all" | "keep-all";
  overflowWrap?: "normal" | "break-word" | "anywhere";
  lineClamp?: number;
  textOverflow?: "clip" | "ellipsis";
  overflowX?: "auto";
  overflowY?: "auto";
  overscrollBehavior?: "auto" | "contain" | "none";
  scrollSnapStop?: "normal" | "always";
  scrollSnapType?: "none" | "x" | "y";
  scrollSnapAlign?: "none" | "start" | "center" | "end";
  placeholderColor?: string;
  objectFit?: "cover" | "contain" | "fill";
  objectPosition?: string;
}

export type ViewStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "flexWrap" | "alignItems" | "justifyContent" | "alignContent" | "flexDirection" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "userSelect" | "textAlign" | "textDecorationLine" | "textDecorationStyle" | "textDecorationColor" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "gridTemplateColumns" | "gridTemplateRows" | "gridAutoColumns" | "gridAutoRows" | "gridAutoFlow" | "justifyItems">;
export type ViewProps = Omit<Props, "style" | "children"> & { style?: ViewStyle; children?: Child; onPage?: (direction: "previous" | "next") => void; onContextMenu?: (event: ContextMenuEvent) => void; anchor?: string; onPointer?: (event: PointerEvent) => void; pointerAxis?: "x" | "y" | "both"; onLayout?: (box: BoxSize) => void; box?: Box; "aria-label"?: string; popover?: boolean | "auto" | "manual" | "hint"; open?: boolean; onToggle?: (open: boolean) => void; drag?: "x" | "y"; dragSnapPoints?: readonly (number | `${number}%`)[]; bind?: Local<number>; onDragStart?: () => void; onDragEnd?: (info: { index: number; velocity: number }) => void; "aria-actions"?: string; onPointerEnter?: () => void; onPointerLeave?: () => void; closeAfter?: number; onCloseRequest?: () => boolean | void; };
export type PageStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "flexWrap" | "alignItems" | "justifyContent" | "alignContent" | "flexDirection" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "userSelect" | "textAlign" | "textDecorationLine" | "textDecorationStyle" | "textDecorationColor" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "gridTemplateColumns" | "gridTemplateRows" | "gridAutoColumns" | "gridAutoRows" | "gridAutoFlow" | "justifyItems">;
export type PageProps = Omit<Props, "style" | "children"> & { style?: PageStyle; children?: Child; "aria-label"?: string; };
export type TextStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "userSelect" | "textAlign" | "textDecorationLine" | "textDecorationStyle" | "textDecorationColor" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "backgroundClip" | "lineClamp" | "textOverflow">;
export type TextProps = Omit<Props, "style" | "children"> & { style?: TextStyle; children?: RunChild; onContextMenu?: (event: ContextMenuEvent) => void; onLayout?: (box: BoxSize) => void; box?: Box; "aria-label"?: string; };
export type ButtonStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "flexWrap" | "alignItems" | "justifyContent" | "alignContent" | "flexDirection" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "userSelect" | "textAlign" | "textDecorationLine" | "textDecorationStyle" | "textDecorationColor" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "gridTemplateColumns" | "gridTemplateRows" | "gridAutoColumns" | "gridAutoRows" | "gridAutoFlow" | "justifyItems">;
export type ButtonProps = Omit<Props, "style" | "children"> & { style?: ButtonStyle; children?: Child; onContextMenu?: (event: ContextMenuEvent) => void; onPointer?: (event: PointerEvent) => void; pointerAxis?: "x" | "y" | "both"; onClick?: () => void; disabled?: boolean; "aria-label"?: string; popoverTarget?: string; interestFor?: string; keepOpen?: boolean; drag?: "x" | "y"; dragSnapPoints?: readonly (number | `${number}%`)[]; bind?: Local<number>; onDragStart?: () => void; onDragEnd?: (info: { index: number; velocity: number }) => void; "aria-actions"?: string; onPointerEnter?: () => void; onPointerLeave?: () => void; };
export type ScrollStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "flexWrap" | "alignItems" | "justifyContent" | "alignContent" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "userSelect" | "textAlign" | "textDecorationLine" | "textDecorationStyle" | "textDecorationColor" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "overflowX" | "overflowY" | "overscrollBehavior" | "scrollSnapStop" | "scrollSnapType">;
export type ScrollProps = Omit<Props, "style" | "children"> & { style?: ScrollStyle; children?: Child; onLayout?: (box: BoxSize) => void; box?: Box; onReachStart?: () => void; onReachEnd?: () => void; onScrollEdge?: (event: ScrollEdgeEvent) => void; bind?: Local<number>; };
export type ListStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "userSelect" | "textAlign" | "textDecorationLine" | "textDecorationStyle" | "textDecorationColor" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "overscrollBehavior" | "scrollSnapStop" | "scrollSnapType">;
export type ListProps = Omit<Props, "style" | "children"> & { style?: ListStyle; children?: never; count: number; row: (index: number) => JSX.Element; height?: (index: number) => number; anchor?: "end" | { index: number; align: "start" | "end"; seq?: number; duration?: number }; itemKeys?: readonly string[]; onReachStart?: () => void; onReachEnd?: () => void; onScrollEdge?: (event: ScrollEdgeEvent) => void; "aria-label"?: string; };
export type ImageStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "objectFit" | "objectPosition">;
export type ImageProps = Omit<Props, "style" | "children"> & { style?: ImageStyle; children?: never; src: string; placeholder?: string; alt?: string; retryKey?: string; onLoad?: () => void; onError?: () => void; };
export type InputStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "userSelect" | "textAlign" | "textDecorationLine" | "textDecorationStyle" | "textDecorationColor" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "placeholderColor" | "textOverflow">;
export type InputProps = Omit<Props, "style" | "children"> & { style?: InputStyle; children?: never; onKeyDown?: EditorKeys; readOnly?: boolean; onFocus?: () => void; onBlur?: () => void; "aria-label"?: string; placeholder?: string; type?: "text" | "password" | "email" | "number" | "tel" | "search" | "date" | "time"; inputMode?: "text" | "numeric" | "decimal" | "tel" | "email"; autoFocus?: boolean; disabled?: boolean; onSubmit?: (value: string) => void; maxLength?: number; autoComplete?: AutoComplete; enterKeyHint?: "enter" | "done" | "go" | "next" | "previous" | "search" | "send"; autoCapitalize?: "none" | "sentences" | "words" | "characters" | "off" | "on"; spellCheck?: boolean; min?: number | string; max?: number | string; step?: number; } & ({ bind: Local<string>; value?: never; onInput?: never } | { bind?: never; value?: string; onInput?: (value: string) => void });
export type TextareaStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "userSelect" | "textAlign" | "textDecorationLine" | "textDecorationStyle" | "textDecorationColor" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "placeholderColor">;
export type TextareaProps = Omit<Props, "style" | "children"> & { style?: TextareaStyle; children?: never; onKeyDown?: EditorKeys; readOnly?: boolean; onFocus?: () => void; onBlur?: () => void; rows?: number; maxRows?: number; "aria-label"?: string; placeholder?: string; inputMode?: "text" | "numeric" | "decimal" | "tel" | "email"; autoFocus?: boolean; disabled?: boolean; onSubmit?: (value: string) => void; maxLength?: number; autoComplete?: AutoComplete; enterKeyHint?: "enter" | "done" | "go" | "next" | "previous" | "search" | "send"; autoCapitalize?: "none" | "sentences" | "words" | "characters" | "off" | "on"; spellCheck?: boolean; } & ({ bind: Local<string>; value?: never; onInput?: never } | { bind?: never; value?: string; onInput?: (value: string) => void });
export type CheckboxStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "flexWrap" | "alignItems" | "justifyContent" | "alignContent" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "userSelect" | "textAlign" | "textDecorationLine" | "textDecorationStyle" | "textDecorationColor" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap">;
export type CheckboxProps = Omit<Props, "style" | "children"> & { style?: CheckboxStyle; children?: Child; indeterminate?: boolean; defaultChecked?: boolean; "aria-label"?: string; disabled?: boolean; } & ({ bind: Local<boolean>; value?: never; onChange?: never } | { bind?: never; value?: boolean; onChange?: (value: ReadonlyData<boolean>) => void });
export type SwitchStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "userSelect" | "textAlign" | "textDecorationLine" | "textDecorationStyle" | "textDecorationColor" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap">;
export type SwitchProps = Omit<Props, "style" | "children"> & { style?: SwitchStyle; children?: Child; defaultChecked?: boolean; "aria-label"?: string; disabled?: boolean; } & ({ bind: Local<boolean>; value?: never; onChange?: never } | { bind?: never; value?: boolean; onChange?: (value: ReadonlyData<boolean>) => void });
export type RadioStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "userSelect" | "textAlign" | "textDecorationLine" | "textDecorationStyle" | "textDecorationColor" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap">;
export type RadioProps = Omit<Props, "style" | "children"> & { style?: RadioStyle; children?: never; options: readonly RadioOption[]; "aria-label"?: string; disabled?: boolean; } & ({ bind: Local<string>; value?: never; onChange?: never } | { bind?: never; value?: string; onChange?: (value: string) => void });
export type SliderStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "color">;
export type SliderProps = Omit<Props, "style" | "children"> & { style?: SliderStyle; children?: never; min?: number; max?: number; step?: number; defaultValue?: number; secondaryValue?: number; "aria-label": string; "aria-valuetext"?: string; disabled?: boolean; onPressChange?: (pressed: boolean) => void; } & ({ bind: Local<number>; value?: never; onChange?: never; onChangeEnd?: (value: ReadonlyData<number>) => void; values?: never; thumbLabels?: never } | { bind?: never; value?: number; onChange?: (value: ReadonlyData<number>) => void; onChangeEnd?: (value: ReadonlyData<number>) => void; values?: never; thumbLabels?: never } | { bind?: never; value?: never; values: readonly [number, number]; thumbLabels: readonly [string, string]; onChange?: (values: readonly [number, number]) => void; onChangeEnd?: (values: readonly [number, number]) => void });
export type DialogStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "flexWrap" | "alignItems" | "justifyContent" | "alignContent" | "flexDirection" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "userSelect" | "textAlign" | "textDecorationLine" | "textDecorationStyle" | "textDecorationColor" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "gridTemplateColumns" | "gridTemplateRows" | "gridAutoColumns" | "gridAutoRows" | "gridAutoFlow" | "justifyItems">;
export type DialogProps = Omit<Props, "style" | "children"> & { style?: DialogStyle; children?: Child; open?: boolean; onClose?: () => void; closedby?: "any" | "closerequest" | "none"; "aria-label": string; drag?: "x" | "y"; dragSnapPoints?: readonly (number | `${number}%`)[]; bind?: Local<number>; onDragStart?: () => void; onDragEnd?: (info: { index: number; velocity: number }) => void; };
export type PlotStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign">;
export type PlotProps = Omit<Props, "style" | "children"> & { style?: PlotStyle; children?: never; onLayout?: (box: BoxSize) => void; box?: Box; kind: "bar" | "line" | "pie"; values: readonly number[]; colors: readonly string[]; "aria-label": string; smooth?: boolean; area?: boolean; selectedIndex?: number; domainMin?: number; domainMax?: number; onSelect?: (index: number) => void; pieLayout?: PieLayout; markerEmphasisScale?: number; };
export type VectorStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "color" | "objectFit">;
export type VectorProps = Omit<Props, "style" | "children"> & { style?: VectorStyle; children?: never; onPointer?: (event: PointerEvent) => void; pointerAxis?: "x" | "y" | "both"; onLayout?: (box: BoxSize) => void; box?: Box; paths: ReadonlyArray<VectorPath>; gradients?: Readonly<Record<string, VectorGradient>>; viewBox?: readonly [number, number, number, number]; };
export type SpanStyle = Pick<Style, "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "letterSpacing" | "textDecorationLine" | "textDecorationStyle" | "textDecorationColor" | "fontVariantNumeric" | "backgroundColor">;
export type SpanProps = Omit<Props, "style" | "children"> & { style?: SpanStyle; children?: RunChild };
export type LinkStyle = Pick<Style, "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "letterSpacing" | "textDecorationLine" | "textDecorationStyle" | "textDecorationColor" | "fontVariantNumeric" | "backgroundColor">;
export type LinkProps = Omit<Props, "style" | "children"> & { style?: LinkStyle; children?: RunChild; onContextMenu?: (event: ContextMenuEvent) => void; onClick?: () => void; "aria-label"?: string; disabled?: boolean; popoverTarget?: string; interestFor?: string; };
export type SelectStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "userSelect" | "textAlign" | "textDecorationLine" | "textDecorationStyle" | "textDecorationColor" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "backgroundClip" | "lineClamp" | "textOverflow" | "placeholderColor">;
export type SelectProps = Omit<Props, "style" | "children"> & { style?: SelectStyle; children?: never; options: readonly RadioOption[]; defaultValue?: string; placeholder?: string; "aria-label"?: string; disabled?: boolean; } & ({ bind: Local<string>; value?: never; onChange?: never } | { bind?: never; value?: string; onChange?: (value: string) => void });
export type VideoStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign" | "objectFit" | "objectPosition">;
export type VideoProps = Omit<Props, "style" | "children"> & { style?: VideoStyle; children?: never; src?: string; sources?: readonly VideoSource[]; player?: Video; poster?: string; autoPlay?: boolean; paused?: boolean; muted?: boolean; loop?: boolean; playbackRate?: number; volume?: number; "aria-label"?: string; onLoadedMetadata?: (event: VideoMetadata) => void; onPlay?: () => void; onPlaying?: () => void; onPause?: (event: VideoPause) => void; onWaiting?: () => void; onSeeked?: (event: { readonly currentTime: number }) => void; onTimeUpdate?: (event: { readonly currentTime: number }) => void; onEnded?: () => void; onError?: (error: VideoError) => void; onRateChange?: (event: { readonly playbackRate: number }) => void; onVolumeChange?: (event: { readonly volume: number; readonly muted: boolean }) => void; onQualityChange?: (event: VideoQualityChange) => void; onFullscreenChange?: (event: { readonly fullscreen: boolean }) => void; };
export type WebviewStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "positionArea" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "ring" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "filter" | "display" | "touchTarget" | "scrollSnapAlign">;
export type WebviewProps = Omit<Props, "style" | "children"> & { style?: WebviewStyle; children?: never; src?: string; web?: WebView; "aria-label": string; handlesDrags?: boolean; onFullscreenChange?: (event: { readonly fullscreen: boolean }) => void; onLoad?: (event: WebViewLoad) => void; onMessage?: (message: WebViewMessage) => void; onBlocked?: (event: WebViewBlocked) => void; onError?: (error: WebViewError) => void; };

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
