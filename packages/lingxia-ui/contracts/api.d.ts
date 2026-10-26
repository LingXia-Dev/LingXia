// Hook, component and data types of the authoring API. The generator appends
// element props and the CSS-named Style from schema.json.

export type Child = JSX.Element | string | number | boolean | null | undefined | readonly Child[];
export type TextChild = string | number | boolean | null | undefined | readonly TextChild[];
export type ComponentResult = JSX.Element | null | undefined | boolean;
export type Component<P = {}> = (props: P) => ComponentResult;
export type ComponentProps<C> = C extends (props: infer P) => unknown ? P : never;
export declare function childrenToArray(children: Child): readonly Exclude<Child, readonly Child[] | boolean | null | undefined>[];

/** Props every element accepts; `style` is narrowed per element. */
export interface Props { style?: Style; className?: string; key?: string; testId?: string; children?: Child }
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
