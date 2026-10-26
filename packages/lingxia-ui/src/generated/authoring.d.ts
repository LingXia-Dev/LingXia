// Generated from packages/lingxia-ui/contracts/schema.json; do not edit.

export type Child = JSX.Element | string | number | boolean | null | undefined | readonly Child[];
export type TextChild = string | number | boolean | null | undefined | readonly TextChild[];
/** Text content of `<text>` and `<span>`: strings, `<span>` and `<link>` runs. */
export type RunChild = TextChild | JSX.Element | readonly RunChild[];
export type ComponentResult = JSX.Element | null | undefined | boolean;
export type Component<P = {}> = (props: P) => ComponentResult;
export type ComponentProps<C> = C extends (props: infer P) => unknown ? P : never;
export declare function childrenToArray(children: Child): readonly Exclude<Child, readonly Child[] | boolean | null | undefined>[];

/** Props every element accepts; `style` is narrowed per element. */
/** `dir` sets the inline direction of a subtree; otherwise it follows `env.direction`. */
export interface Props {
  style?: Style; className?: string; key?: string; testId?: string; dir?: "ltr" | "rtl"; children?: Child;
  /** Semantic role; controls keep their own. Headings take `headingLevel` 1..6. */
  role?: "heading" | "list" | "listitem" | "dialog" | "alert" | "status"; headingLevel?: 1 | 2 | 3 | 4 | 5 | 6;
  /** Removes the node and its subtree from accessibility. */
  accessibilityHidden?: boolean;
  /** Announce changes; `alert` is assertive and `status` polite unless set. */
  accessibilityLiveRegion?: "off" | "polite" | "assertive";
}
export type LeafProps = Omit<Props, "children"> & { children?: never };

export interface Env {
  readonly theme: "light" | "dark";
  readonly viewport: Readonly<{ width: number; height: number }>;
  readonly insets: Readonly<{ top: number; right: number; bottom: number; left: number }>;
  readonly keyboard: Readonly<{ height: number }>;
  /** `fine` pointers (mouse, trackpad) hover; `coarse` ones (touch) do not. */
  readonly pointer: "fine" | "coarse";
  readonly os: string;
  readonly formFactor: string;
  readonly sizeClass: string;
  readonly displayLanguage: string;
  /** The system asks for reduced motion; `motion-reduce:`/`motion-safe:` follow it. */
  readonly reducedMotion: boolean;
  /** Inline direction derived from `displayLanguage`. */
  readonly direction: "ltr" | "rtl";
  /** Whether the window can dock an `as: 'aside'` beside the main; false until the host reports it. */
  readonly canDock: boolean;
  readonly runner: boolean;
  /** `fallback: "driver"` when this device's GPU driver failed and pages render on the CPU. */
  readonly renderer?: Readonly<{ fallback: "driver" | null }>;
}

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
export interface Local<T> { readonly [localCell]: true; readonly value: ReadonlyData<T>; set(value: ReadonlyData<LocalData<T>>): void }
export declare function useLocal<T>(name: string, initial: T, ...invalid: [T] extends [LocalData<T>] ? [] : [never]): Local<T>;

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

export type LeaveReason = "back" | "home";
export interface LeaveGuard {
  readonly pending: LeaveReason | null; readonly requests: number; readonly error: ResourceError | null;
  answer(leave: boolean): void; request(reason: LeaveReason): void;
}
export declare function useLeaveGuard(name: string, options: { enabled: boolean }): LeaveGuard;

export interface DataListProps<T> extends Omit<ListProps, "count" | "itemKeys" | "row" | "height"> {
  items: readonly T[];
  getKey: (item: T, index: number) => string;
  renderItem: (item: T, index: number) => JSX.Element;
  getItemHeight?: (item: T, index: number) => number;
}
export declare function List<T>(props: DataListProps<T>): JSX.Element;

export interface BoxShadow { offsetX?: number; offsetY?: number; blur?: number; spread?: number; color?: string }
export interface GradientStop { offset: number; color: string }
/** Positions are fractions of the box; conic angles are degrees clockwise from the top. */
export type BackgroundGradient =
  | { kind?: "linear"; direction?: "top" | "right" | "bottom" | "left" | "top-right" | "bottom-right" | "bottom-left" | "top-left"; angle?: number; colorSpace?: "srgb" | "oklab"; stops: ReadonlyArray<GradientStop> }
  | { kind: "radial"; shape?: "circle" | "ellipse"; size?: "closest-side" | "closest-corner" | "farthest-side" | "farthest-corner"; at?: readonly [number, number]; colorSpace?: "srgb" | "oklab"; stops: ReadonlyArray<GradientStop> }
  | { kind: "conic"; from?: number; at?: readonly [number, number]; colorSpace?: "srgb" | "oklab"; stops: ReadonlyArray<GradientStop> };
export interface PieLayout { center: readonly [number, number]; innerRadius: number; outerRadius: number; emphasisScale?: number }
/** `fill`/`stroke` take a hex color, `currentColor`, `none` or `url(#id)` naming one of the vector's `gradients`. */
export interface VectorPath {
  d: string; fill?: string; stroke?: string; strokeWidth?: number; strokeDasharray?: readonly number[]; strokeDashoffset?: number;
  strokeLinecap?: "butt" | "round" | "square"; strokeLinejoin?: "miter" | "round" | "bevel";
  /** An SVG transform list: matrix, translate, scale, rotate, skewX, skewY. */
  transform?: string; vectorEffect?: "none" | "non-scaling-stroke";
}
/** SVG gradient; coordinates default to `objectBoundingBox` units. */
export interface VectorGradient {
  type: "linear" | "radial"; x1?: number; y1?: number; x2?: number; y2?: number;
  cx?: number; cy?: number; r?: number; fx?: number; fy?: number; fr?: number;
  units?: "objectBoundingBox" | "userSpaceOnUse"; spread?: "pad" | "reflect" | "repeat";
  stops: ReadonlyArray<{ offset: number; color: string; opacity?: number }>;
}
export interface RadioOption { readonly value: string; readonly label: string; readonly disabled?: boolean; readonly testId?: string }
/** HTML autocomplete tokens; Android maps them to autofill hints. */
export type AutoComplete = "off" | "on" | "username" | "current-password" | "new-password" | "one-time-code" | "email" | "tel" | "name" | "given-name" | "family-name" | "street-address" | "postal-code" | "country" | "cc-number" | "cc-exp" | "cc-csc" | "bday";

/** CSS-named style; each element accepts the subset its `*Style` type lists. */
export interface Style {
  width?: number | `${number}%` | `${number}vw` | `${number}vh` | "auto" | "fit-content" | "max-content" | "min-content";
  height?: number | `${number}%` | `${number}vw` | `${number}vh` | "auto" | "fit-content" | "max-content" | "min-content";
  minWidth?: number | `${number}%` | `${number}vw` | `${number}vh`;
  minHeight?: number | `${number}%` | `${number}vw` | `${number}vh`;
  maxWidth?: number | `${number}%` | `${number}vw` | `${number}vh` | `${number}ch`;
  aspectRatio?: number | "auto";
  maxHeight?: number | `${number}%` | `${number}vw` | `${number}vh`;
  margin?: number | "auto";
  marginTop?: number | "auto";
  marginRight?: number | "auto";
  marginBottom?: number | "auto";
  marginLeft?: number | "auto";
  marginInlineStart?: number | "auto";
  marginInlineEnd?: number | "auto";
  position?: "relative" | "absolute" | "fixed";
  inset?: number | `${number}%`;
  top?: number | `${number}%`;
  right?: number | `${number}%`;
  bottom?: number | `${number}%`;
  left?: number | `${number}%`;
  insetInlineStart?: number | `${number}%`;
  insetInlineEnd?: number | `${number}%`;
  zIndex?: number;
  flexGrow?: number;
  flexShrink?: number;
  flexBasis?: number | `${number}%` | "auto";
  alignSelf?: "flex-start" | "start" | "center" | "flex-end" | "end" | "stretch" | "baseline";
  gridColumn?: number | string;
  gridColumnStart?: number | string;
  gridColumnEnd?: number | string;
  gridRow?: number | string;
  gridRowStart?: number | string;
  gridRowEnd?: number | string;
  justifySelf?: "start" | "center" | "end" | "stretch" | "baseline";
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
  backdropFilter?: string;
  translate?: number | `${number}%` | readonly [number | `${number}%`, number | `${number}%`];
  rotate?: number;
  scale?: number | readonly [number, number];
  transformOrigin?: readonly [number, number];
  padding?: number;
  paddingTop?: number;
  paddingRight?: number;
  paddingBottom?: number;
  paddingLeft?: number;
  paddingInlineStart?: number;
  paddingInlineEnd?: number;
  display?: "flex" | "grid" | "none" | "block" | "inline-flex" | "inline-block";
  flexDirection?: "row" | "column" | "row-reverse" | "column-reverse";
  alignItems?: "flex-start" | "start" | "center" | "flex-end" | "end" | "stretch" | "baseline";
  justifyContent?: "flex-start" | "start" | "center" | "flex-end" | "end" | "space-between" | "space-around" | "space-evenly";
  alignContent?: "flex-start" | "start" | "center" | "flex-end" | "end" | "space-between" | "space-around" | "space-evenly" | "stretch";
  flexWrap?: "nowrap" | "wrap" | "wrap-reverse";
  gap?: number;
  rowGap?: number;
  columnGap?: number;
  gridTemplateColumns?: number | string;
  gridTemplateRows?: number | string;
  gridAutoColumns?: string;
  gridAutoRows?: string;
  gridAutoFlow?: "row" | "column" | "row dense" | "column dense";
  justifyItems?: "start" | "center" | "end" | "stretch" | "baseline";
  color?: string;
  fontSize?: number;
  fontWeight?: number;
  fontFamily?: "sans-serif" | "monospace";
  fontStyle?: "normal" | "italic";
  lineHeight?: number | `${number}px`;
  letterSpacing?: number | `${number}em`;
  fontVariantNumeric?: string;
  textAlign?: "start" | "end" | "left" | "center" | "right" | "justify";
  textDecorationLine?: "none" | "line-through" | "underline";
  textTransform?: "none" | "uppercase" | "lowercase" | "capitalize";
  whiteSpace?: "normal" | "nowrap" | "pre" | "pre-wrap" | "pre-line";
  wordBreak?: "normal" | "break-all" | "keep-all";
  overflowWrap?: "normal" | "break-word" | "anywhere";
  lineClamp?: number;
  textOverflow?: "clip" | "ellipsis";
  overflowX?: "auto";
  overflowY?: "auto";
  overscrollBehavior?: "auto" | "contain" | "none";
  placeholderColor?: string;
  objectFit?: "cover" | "contain" | "fill";
}

export type ViewStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "flexWrap" | "alignItems" | "justifyContent" | "alignContent" | "flexDirection" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "gridTemplateColumns" | "gridTemplateRows" | "gridAutoColumns" | "gridAutoRows" | "gridAutoFlow" | "justifyItems">;
export type ViewProps = Omit<Props, "style" | "children"> & { style?: ViewStyle; children?: Child; accessibilityLabel?: string; };
export type ColumnStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "flexWrap" | "alignItems" | "justifyContent" | "alignContent" | "flexDirection" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "gridTemplateColumns" | "gridTemplateRows" | "gridAutoColumns" | "gridAutoRows" | "gridAutoFlow" | "justifyItems">;
export type ColumnProps = Omit<Props, "style" | "children"> & { style?: ColumnStyle; children?: Child; accessibilityLabel?: string; };
export type RowStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "flexWrap" | "alignItems" | "justifyContent" | "alignContent" | "flexDirection" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "gridTemplateColumns" | "gridTemplateRows" | "gridAutoColumns" | "gridAutoRows" | "gridAutoFlow" | "justifyItems">;
export type RowProps = Omit<Props, "style" | "children"> & { style?: RowStyle; children?: Child; accessibilityLabel?: string; };
export type PageStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "flexWrap" | "alignItems" | "justifyContent" | "alignContent" | "flexDirection" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "gridTemplateColumns" | "gridTemplateRows" | "gridAutoColumns" | "gridAutoRows" | "gridAutoFlow" | "justifyItems">;
export type PageProps = Omit<Props, "style" | "children"> & { style?: PageStyle; children?: Child; accessibilityLabel?: string; };
export type TextStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "lineClamp" | "textOverflow">;
export type TextProps = Omit<Props, "style" | "children"> & { style?: TextStyle; children?: RunChild; accessibilityLabel?: string; };
export type ButtonStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "flexWrap" | "alignItems" | "justifyContent" | "alignContent" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap">;
export type ButtonProps = Omit<Props, "style" | "children"> & { style?: ButtonStyle; children?: Child; onClick?: () => void; disabled?: boolean; accessibilityLabel?: string; };
export type SpacerStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display">;
export type SpacerProps = Omit<Props, "style" | "children"> & { style?: SpacerStyle; children?: never };
export type ScrollStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "flexWrap" | "alignItems" | "justifyContent" | "alignContent" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "overflowX" | "overflowY" | "overscrollBehavior">;
export type ScrollProps = Omit<Props, "style" | "children"> & { style?: ScrollStyle; children?: Child; onReachStart?: () => void; onReachEnd?: () => void; };
export type ListStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "overscrollBehavior">;
export type ListProps = Omit<Props, "style" | "children"> & { style?: ListStyle; children?: never; count: number; row: (index: number) => JSX.Element; height?: (index: number) => number; anchor?: "end" | { index: number; align: "start" | "end"; seq?: number }; itemKeys?: readonly string[]; onReachStart?: () => void; onReachEnd?: () => void; };
export type ImageStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "objectFit">;
export type ImageProps = Omit<Props, "style" | "children"> & { style?: ImageStyle; children?: never; src: string; alt?: string; retryKey?: string; onLoad?: () => void; onError?: () => void; };
export type InputStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "placeholderColor">;
export type InputProps = Omit<Props, "style" | "children"> & { style?: InputStyle; children?: never; accessibilityLabel?: string; placeholder?: string; secure?: boolean; keyboard?: "text" | "number" | "email" | "phone" | "date" | "time"; autoFocus?: boolean; disabled?: boolean; onSubmit?: (value: string) => void; maxLength?: number; autoComplete?: AutoComplete; enterKeyHint?: "enter" | "done" | "go" | "next" | "previous" | "search" | "send"; autoCapitalize?: "none" | "sentences" | "words" | "characters" | "off" | "on"; spellCheck?: boolean; min?: number | string; max?: number | string; step?: number; } & ({ bind: Local<string>; value?: never; onInput?: never } | { bind?: never; value?: string; onInput?: (value: string) => void });
export type TextareaStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "placeholderColor">;
export type TextareaProps = Omit<Props, "style" | "children"> & { style?: TextareaStyle; children?: never; accessibilityLabel?: string; placeholder?: string; secure?: boolean; keyboard?: "text" | "number" | "email" | "phone"; autoFocus?: boolean; disabled?: boolean; submitOnEnter?: boolean; onSubmit?: (value: string) => void; maxLength?: number; autoComplete?: AutoComplete; enterKeyHint?: "enter" | "done" | "go" | "next" | "previous" | "search" | "send"; autoCapitalize?: "none" | "sentences" | "words" | "characters" | "off" | "on"; spellCheck?: boolean; } & ({ bind: Local<string>; value?: never; onInput?: never } | { bind?: never; value?: string; onInput?: (value: string) => void });
export type SearchStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap" | "placeholderColor">;
export type SearchProps = Omit<Props, "style" | "children"> & { style?: SearchStyle; children?: never; accessibilityLabel?: string; placeholder?: string; autoFocus?: boolean; disabled?: boolean; onSubmit?: (value: string) => void; maxLength?: number; autoComplete?: AutoComplete; enterKeyHint?: "enter" | "done" | "go" | "next" | "previous" | "search" | "send"; autoCapitalize?: "none" | "sentences" | "words" | "characters" | "off" | "on"; spellCheck?: boolean; } & ({ bind: Local<string>; value?: never; onInput?: never } | { bind?: never; value?: string; onInput?: (value: string) => void });
export type CheckboxStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "flexWrap" | "alignItems" | "justifyContent" | "alignContent" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap">;
export type CheckboxProps = Omit<Props, "style" | "children"> & { style?: CheckboxStyle; children?: Child; accessibilityLabel?: string; disabled?: boolean; } & ({ bind: Local<boolean>; value?: never; onChange?: never } | { bind?: never; value?: boolean; onChange?: (value: ReadonlyData<boolean>) => void });
export type SwitchStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap">;
export type SwitchProps = Omit<Props, "style" | "children"> & { style?: SwitchStyle; children?: TextChild; accessibilityLabel?: string; disabled?: boolean; } & ({ bind: Local<boolean>; value?: never; onChange?: never } | { bind?: never; value?: boolean; onChange?: (value: ReadonlyData<boolean>) => void });
export type RadioStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap">;
export type RadioProps = Omit<Props, "style" | "children"> & { style?: RadioStyle; children?: never; options: readonly RadioOption[]; accessibilityLabel?: string; disabled?: boolean; } & ({ bind: Local<string>; value?: never; onChange?: never } | { bind?: never; value?: string; onChange?: (value: string) => void });
export type SliderStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "color">;
export type SliderProps = Omit<Props, "style" | "children"> & { style?: SliderStyle; children?: never; min?: number; max?: number; step?: number; accessibilityLabel: string; disabled?: boolean; } & ({ bind: Local<number>; value?: never; onChange?: never } | { bind?: never; value?: number; onChange?: (value: ReadonlyData<number>) => void });
export type GridStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "gridTemplateColumns" | "gridTemplateRows" | "gridAutoColumns" | "gridAutoRows" | "gridAutoFlow" | "justifyItems" | "alignItems" | "justifyContent" | "alignContent" | "gap" | "rowGap" | "columnGap" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap">;
export type GridProps = Omit<Props, "style" | "children"> & { style?: GridStyle; children?: Child };
export type StackStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "lineHeight" | "letterSpacing" | "fontVariantNumeric" | "textAlign" | "textDecorationLine" | "textTransform" | "whiteSpace" | "wordBreak" | "overflowWrap">;
export type StackProps = Omit<Props, "style" | "children"> & { style?: StackStyle; children?: Child };
export type PlotStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display">;
export type PlotProps = Omit<Props, "style" | "children"> & { style?: PlotStyle; children?: never; kind: "bar" | "line" | "pie"; values: readonly number[]; colors: readonly string[]; accessibilityLabel: string; smooth?: boolean; area?: boolean; selectedIndex?: number; domainMin?: number; domainMax?: number; onSelect?: (index: number) => void; pieLayout?: PieLayout; markerEmphasisScale?: number; };
export type VectorStyle = Pick<Style, "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "aspectRatio" | "maxHeight" | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" | "marginInlineStart" | "marginInlineEnd" | "position" | "inset" | "top" | "right" | "bottom" | "left" | "insetInlineStart" | "insetInlineEnd" | "zIndex" | "flexGrow" | "flexShrink" | "flexBasis" | "alignSelf" | "gridColumn" | "gridColumnStart" | "gridColumnEnd" | "gridRow" | "gridRowStart" | "gridRowEnd" | "justifySelf" | "order" | "opacity" | "overflow" | "backgroundColor" | "backgroundImage" | "borderRadius" | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomRightRadius" | "borderBottomLeftRadius" | "borderStartStartRadius" | "borderStartEndRadius" | "borderEndStartRadius" | "borderEndEndRadius" | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth" | "borderLeftWidth" | "borderInlineStartWidth" | "borderInlineEndWidth" | "borderStyle" | "borderColor" | "boxShadow" | "backdropFilter" | "translate" | "rotate" | "scale" | "transformOrigin" | "display" | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" | "paddingInlineStart" | "paddingInlineEnd" | "color" | "objectFit">;
export type VectorProps = Omit<Props, "style" | "children"> & { style?: VectorStyle; children?: never; paths: ReadonlyArray<VectorPath>; gradients?: Readonly<Record<string, VectorGradient>>; viewBox?: readonly [number, number, number, number]; };
export type SpanStyle = Pick<Style, "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "textDecorationLine" | "fontVariantNumeric" | "backgroundColor">;
export type SpanProps = Omit<Props, "style" | "children"> & { style?: SpanStyle; children?: RunChild };
export type LinkStyle = Pick<Style, "color" | "fontSize" | "fontWeight" | "fontFamily" | "fontStyle" | "textDecorationLine" | "fontVariantNumeric" | "backgroundColor">;
export type LinkProps = Omit<Props, "style" | "children"> & { style?: LinkStyle; children?: TextChild; onPress?: () => void; accessibilityLabel?: string; disabled?: boolean; };

declare global {
  namespace JSX {
    type ElementType = keyof IntrinsicElements | ((props: any) => ComponentResult);
    interface Element { readonly __ui: unique symbol }
    interface IntrinsicAttributes { key?: string }
    interface ElementChildrenAttribute { children: {} }
    interface IntrinsicElements {
      "view": ViewProps;
      "column": ColumnProps;
      "row": RowProps;
      "page": PageProps;
      "text": TextProps;
      "button": ButtonProps;
      "spacer": SpacerProps;
      "scroll": ScrollProps;
      "list": ListProps;
      "image": ImageProps;
      "input": InputProps;
      "textarea": TextareaProps;
      "search": SearchProps;
      "checkbox": CheckboxProps;
      "switch": SwitchProps;
      "radio": RadioProps;
      "slider": SliderProps;
      "grid": GridProps;
      "stack": StackProps;
      "plot": PlotProps;
      "vector": VectorProps;
      "span": SpanProps;
      "link": LinkProps;
    }
  }
}
