import type { Style } from "../generated/style.js";
export type { Style, BoxShadow, LinearGradient } from "../generated/style.js";

export type Child = JSX.Element | string | number | boolean | null | undefined | readonly Child[];
export type ComponentResult = JSX.Element | null | undefined | boolean;
export type Component<P = {}> = (props: P) => ComponentResult;
export type ComponentProps<C> = C extends (props: infer P, ...args: any[]) => unknown ? P : never;
export declare function childrenToArray(children: Child): readonly Exclude<Child, readonly Child[] | boolean | null | undefined>[];
export interface Props { style?: Style; className?: string; key?: string; testId?: string; children?: Child }
export type TextChild = string | number | boolean | null | undefined | readonly TextChild[];
export type LeafProps = Omit<Props, 'children'> & { children?: never };
export interface Env { readonly theme: "light" | "dark"; readonly viewport: Readonly<{ width: number; height: number }>; readonly insets: Readonly<{ top: number; right: number; bottom: number; left: number }>; readonly keyboard: Readonly<{ height: number }>; readonly os: string; readonly formFactor: string; readonly sizeClass: string; readonly displayLanguage: string; readonly aside: boolean; readonly runner: boolean }
export interface Stream<T = unknown, R = unknown> extends AsyncIterable<ReadonlyData<T>> { readonly id: string; readonly result: Promise<ReadonlyData<R>>; cancel(): void; on(kind: "data", listener: (value: ReadonlyData<T>) => void): this; on(kind: "end", listener: (value: ReadonlyData<R>) => void): this; on(kind: "error", listener: (error: Error & { code?: string }) => void): this; }
export interface Channel<T = unknown, S = unknown> extends AsyncIterable<ReadonlyData<T>> { readonly id: string; send(payload: S): void; close(code?: string, reason?: string): void; on(kind: "data", listener: (value: ReadonlyData<T>) => void): this; on(kind: "close", listener: (code?: string, reason?: string) => void): this; on(kind: "error", listener: (error: Error & { code?: string }) => void): this; }
export interface Channels { open<T = unknown, S = unknown>(topic: string, params?: unknown): Promise<Channel<T, S>>; }
export type ReadonlyData<T> = T extends object ? { readonly [K in keyof T]: ReadonlyData<T[K]> } : T;
export type ViewActions<A> = { readonly [K in keyof A]: A[K] extends (...args: infer P) => Promise<infer R> ? (...args: P) => Promise<ReadonlyData<R>> : A[K] };
export interface View<A = Readonly<Record<string, (payload?: unknown) => Promise<unknown>>>, D = Record<string, unknown>> { readonly data: ReadonlyData<D>; readonly actions: ViewActions<A>; readonly channels: Channels; readonly env: Env }
export interface PieLayout { center: readonly [number, number]; innerRadius: number; outerRadius: number; emphasisScale?: number }
export interface PlotProps extends LeafProps { pieLayout?: PieLayout; markerEmphasisScale?: number; accessibilityLabel: string; kind: "bar" | "line" | "pie"; values: readonly number[]; colors: readonly string[]; smooth?: boolean; area?: boolean; selectedIndex?: number; domainMin?: number; domainMax?: number; onSelect?: (index: number) => void; }
export interface VectorPath { d: string; fill?: string; stroke?: string; strokeWidth?: number; strokeDasharray?: readonly number[]; vectorEffect?: "none" | "non-scaling-stroke" }
export interface VectorProps extends LeafProps { fit?: "contain" | "fill"; viewBox?: readonly [number, number, number, number]; paths: ReadonlyArray<VectorPath> }
export interface ImageProps extends LeafProps { src: string; alt?: string; fit?: "cover" | "contain" | "fill"; retryKey?: string; onLoad?: () => void; onError?: () => void }
export interface ScrollProps extends Props { onReachStart?: () => void; onReachEnd?: () => void }
export interface ListProps extends Omit<ScrollProps, "children"> { children?: never; count: number; itemKeys?: readonly string[]; row: (index: number) => JSX.Element; height?: (index: number) => number; anchor?: "end" | { index: number; align: "start" | "end"; seq?: number } }
export interface DataListProps<T> extends Omit<ListProps, 'count' | 'itemKeys' | 'row' | 'height'> {
  items: readonly T[];
  getKey: (item: T, index: number) => string;
  renderItem: (item: T, index: number) => JSX.Element;
  getItemHeight?: (item: T, index: number) => number;
}
export declare function List<T>(props: DataListProps<T>): JSX.Element;
declare const localCell: unique symbol;
/** JSON-shaped state; undefined object fields are omitted, including optional fields. */
export type LocalData<T> = T extends string | number | boolean | null ? T
  : T extends (...args: any[]) => any ? never
  : T extends readonly unknown[] ? { [K in keyof T]: LocalData<T[K]> }
  : T extends object ? { [K in keyof T]: T[K] extends undefined ? undefined : LocalData<T[K]> } : never;
export interface Local<T> { readonly [localCell]: true; readonly value: ReadonlyData<T>; set(value: ReadonlyData<LocalData<T>>): void }
export type InputBinding =
  | { bind: Local<string>; value?: never; onInput?: never }
  | { bind?: never; value?: string; onInput?: (value: string) => void };
export type ChangeBinding<T> =
  | { bind: Local<T>; value?: never; onChange?: never }
  | { bind?: never; value?: T; onChange?: (value: ReadonlyData<T>) => void };
export type InputProps = LeafProps & InputOptions & InputBinding;
export interface InputOptions { accessibilityLabel?: string; placeholder?: string; secure?: boolean; keyboard?: "text" | "number" | "email" | "phone" | "search"; autoFocus?: boolean; enabled?: boolean; onSubmit?: (value: string) => void }
export type TextareaProps = InputProps & { submitOnEnter?: boolean };
export type SearchProps = Omit<LeafProps & InputOptions, "secure" | "keyboard"> & InputBinding;
export interface TextProps extends Omit<Props, "children"> { children?: TextChild; lines?: number }
export interface GridProps extends Props { columns: number }
export type SliderProps = LeafProps & ChangeBinding<number> & { min?: number; max?: number; step?: number; enabled?: boolean; accessibilityLabel: string }
export interface RadioOption { readonly value: string; readonly label: string; readonly enabled?: boolean; readonly testId?: string }
export type RadioProps = LeafProps & ChangeBinding<string> & { options: readonly RadioOption[]; enabled?: boolean; accessibilityLabel?: string };
export type CheckboxProps = CheckProps & { accessibilityLabel?: string };
export type CheckProps = Props & ChangeBinding<boolean> & { enabled?: boolean };
export interface ButtonProps extends Props { onClick?: () => void; enabled?: boolean; accessibilityLabel?: string }
export interface ChannelResource<S = unknown> { readonly connecting: boolean; readonly connected: boolean; readonly error: string | null; connect(): Promise<void>; close(): void; send(payload: S): void; }
export declare function useChannel<T = unknown, S = unknown>(name: string, topic: string, options: { params?: unknown; onData: (message: ReadonlyData<T>) => void }): ChannelResource<S>;
export type LeaveReason = "back" | "home";
export interface LeaveGuard { readonly pending: LeaveReason | null; readonly requests: number; readonly error: string | null; answer(leave: boolean): void; request(reason: LeaveReason): void; }
export declare function useLeaveGuard(name: string, enabled: boolean): LeaveGuard;
export declare function useLocal<T>(name: string, initial: T, ...invalid: [T] extends [LocalData<T>] ? [] : [never]): Local<T>;
export interface RequestResource<T> { readonly pending: boolean; readonly value: ReadonlyData<T> | null; readonly error: string | null; reload(): Promise<void>; }
export declare function useRequest<P extends [] | [unknown?], R>(name: string, action: (...params: P) => Promise<R>, ...params: P): RequestResource<R>;
export declare function useRequest<T = unknown>(name: string, method: string, params?: unknown): RequestResource<T>;
export interface StreamResource<R> { readonly status: "idle" | "streaming" | "completed" | "canceled" | "failed"; readonly error: string | null; readonly result: ReadonlyData<R> | null; start(params?: unknown): void; cancel(): void; }
export interface TypedStreamResource<P extends unknown[], R> extends Omit<StreamResource<R>, 'start'> { start(...params: P): void }
export declare function useStream<P extends [] | [unknown?], T, R>(name: string, action: (...params: P) => Stream<T, R>, options: { onData: (value: ReadonlyData<T>) => void }): TypedStreamResource<P, R>;
export declare function useStream<T = unknown, R = unknown>(name: string, method: string, options: { onData: (value: ReadonlyData<T>) => void }): StreamResource<R>;
export declare function useView<T = View>(): T;
export declare function h(tag: string | Function, props: Props | null, ...children: Child[]): JSX.Element;
declare global {
  namespace JSX {
    type ElementType = keyof IntrinsicElements | ((props: any, ...args: any[]) => ComponentResult);
    interface Element { readonly __ui: unique symbol }
    interface IntrinsicAttributes { key?: string }
    interface ElementChildrenAttribute { children: {} }
    interface IntrinsicElements {
      view: Props;
      column: Props;
      row: Props;
      text: TextProps;
      button: ButtonProps;
      spacer: LeafProps;
      scroll: ScrollProps;
      list: ListProps;
      image: ImageProps;
      input: InputProps;
      textarea: TextareaProps;
      checkbox: CheckboxProps;
      switch: CheckProps;
      radio: RadioProps;
      search: SearchProps;
      slider: SliderProps;
      grid: GridProps;
      stack: Props;
      plot: PlotProps;
      vector: VectorProps;
    }
  }
}
