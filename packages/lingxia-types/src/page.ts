/**
 * `T` where every value survives the JSON transfer of page data to the
 * View; a function, Date, Map, Set, promise or class instance becomes
 * `never`, so a contract holding one does not compile. `undefined` members
 * are allowed: the transfer drops them, as an optional member reads.
 */
export type JsonData<T> =
  0 extends 1 & T ? any
    : unknown extends T ? unknown
    : T extends string | number | boolean | null | undefined ? T
    : T extends (...args: never[]) => unknown ? never
    : T extends Date | RegExp | Map<unknown, unknown> | Set<unknown> | PromiseLike<unknown> | symbol | bigint ? never
    : T extends readonly unknown[] ? { [K in keyof T]: JsonData<T[K]> }
    : T extends object ? { [K in keyof T]: JsonData<T[K]> }
    : never;

/**
 * A shared, type-only page contract: the page's `data` and public actions,
 * written once and used by Logic, View and tests. Importing it installs no
 * Logic globals. `Data` must be JSON (see `JsonData`); otherwise the
 * contract has no `data`, and every use of it names the problem.
 */
export type PageContract<Data = unknown, Actions = unknown> =
  [Data] extends [JsonData<Data>]
    ? { readonly data: Data; readonly actions: Actions }
    : { readonly __pageDataMustBeJson: "PageContract data must be JSON: no functions, Dates, Maps, Sets or class instances" };

import type {
  DeepReadonly,
  JsonValue,
  NavigationBarPatch,
  PageLoadOptions,
  PageEnvContext,
  MessageIn,
  MessageOut,
  PageSurface,
  SignalOptions,
  VideoContext,
} from './generated/logic.js';

declare const typeTag: unique symbol;

/** A type a page declares: `params: type<{ id: string }>()`. Nothing at run time. */
export interface Declared<T> {
  readonly [typeTag]?: T;
}

/** Declare a page's `params`, `result` or `messages` type. */
export function type<T>(): Declared<T> {
  return undefined as unknown as Declared<T>;
}

/** Lifecycle hook names a page's `setup` may return. */
export type PageHookName = 'onLoad' | 'onShow' | 'onReady' | 'onHide' | 'onUnload' | 'onPullDownRefresh';

type Hook = () => void | Promise<void>;

/** What `setup` returns: lifecycle hooks under their names, and `actions`. */
export interface PageSetup<Params, Actions> {
  onLoad?: (params: Params) => void | Promise<void>;
  onShow?: Hook;
  onReady?: Hook;
  onHide?: Hook;
  onUnload?: Hook;
  /** A pull, or `page.refresh()`. The indicator stays until the returned promise settles. */
  onPullDownRefresh?: Hook;
  /** The page's whole View-callable surface. Actions are called with `this` undefined. */
  actions?: Actions & ThisType<void>;
}

type ExactSetup<R> = {
  [K in keyof R]: K extends PageHookName | 'actions'
    ? R[K]
    : Lowercase<K & string> extends Lowercase<PageHookName>
      ? { 'LingXia type error': `'${K & string}' differs only in case from a lifecycle hook` }
      : { 'LingXia type error': `'${K & string}' is not a lifecycle hook; put View-callable functions under actions` };
};

type SyncSetup<R> = R extends PromiseLike<unknown>
  ? { 'LingXia type error': 'setup is synchronous; start async work in onLoad' }
  : unknown;

/**
 * A recipe changes the draft in place. What it returns is ignored, so
 * `d => d.list.push(x)` is fine; a promise, or a new object shaped like the
 * data, is refused.
 */
export type UpdateRecipe<Data, R = unknown> = (draft: Data) => R & RecipeReturn<R, Data>;

type RecipeReturn<R, Data> = R extends PromiseLike<unknown>
  ? { 'LingXia type error': 'a recipe is synchronous; await first, then update' }
  : R extends readonly unknown[] ? unknown
  : R extends object
    ? R extends Data ? { 'LingXia type error': 'change the draft in place; a returned object is not applied' } : unknown
    : unknown;

/** One page instance, as `setup` and everything it returns see it. */
export interface PageApi<Data, Params, Result, Messages = never> {
  /** The current data. Readonly: change it with `update`. */
  readonly data: DeepReadonly<Data>;
  /**
   * The one way to change data: `page.update(d => { d.n += 1; })`. `page.data`
   * changes at once; the View gets the changes once per turn. Throws inside
   * another `update`; ignored after the page unloads.
   */
  update<R>(recipe: UpdateRecipe<Data, R>): void;
  /**
   * Resolves when the View has everything updated so far. Rejects on unload.
   * Throws while `setup` runs; call it from a hook or an action.
   */
  flush(): Promise<void>;
  /** The params this instance was opened with. */
  readonly params: Params;
  /** This page's name in `lxapp.json`. */
  readonly name: string;
  /** Aborted as the page unloads, before `onUnload`. */
  readonly signal: AbortSignal;
  /**
   * Leave, with `value` as the opener's result. A pushed page goes back; a
   * surface page closes its surface. Throws for a root or tab page, or a
   * pushed page that is not on top, and while `setup` runs.
   */
  close(value?: Result): Promise<void>;
  /** What the opener's result is when this page leaves another way (Back, a dismiss). Throws while `setup` runs. */
  setResult(value: Result): void;
  /**
   * The page that opened this one, by `lx.nav.go` or `lx.surface.openPage`;
   * absent for a root, a tab, after a restart, and while `setup` runs (read
   * it in a hook or an action).
   */
  readonly opener?: {
    /** Delivers to the opener's `onMessage` handlers; until it first subscribes, messages wait. */
    send(message: MessageOut<Messages>): void;
    /** Receives what the opener sends. Returns a function that removes the handler. */
    onMessage(handler: (message: MessageIn<Messages>) => void, options?: SignalOptions): () => void;
  };
  /** This page's surface when it was opened by `lx.surface.openPage`; absent while `setup` runs. */
  readonly surface?: Omit<PageSurface, 'opened' | 'result' | 'send' | 'onMessage'>;
  /** This page's navigation bar, reached even while another page is on top. */
  readonly navigationBar: {
    /** Patch it; unset fields stay as they are. */
    update(patch: NavigationBarPatch): Promise<void>;
  };
  /**
   * Show the refresh indicator and run `onPullDownRefresh`, as a pull would.
   * Throws `E_INVALID_STATE` unless the page config sets `enablePullDownRefresh`.
   */
  refresh(): void;
  /** The player named `name` on this page: a `useVideo` name or `<video id>`, or an `<lx-video>` id. */
  video(name: string): VideoContext;
  /**
   * The size class this page lays out for. `watch` delivers it at once, then
   * when `sizeClass` or `canDock` changes (not on every resize), and stops
   * when the page unloads.
   */
  readonly env: PageEnvContext & {
    watch(callback: (env: PageEnvContext) => void, options?: SignalOptions): () => void;
  };
}

type ViewCallable<A> = {
  readonly [K in keyof A]: A[K] extends (...args: infer P) => infer R
    ? R extends AsyncIterable<unknown> ? A[K] : (...args: P) => Promise<Awaited<R>>
    : never;
};

/**
 * What `definePage` returns, the page's type. `View<typeof page>` and
 * `t.app.page<typeof page>()` read `data` and `actions` from it; `lx.nav`
 * reads params and result. `data` holds the initial data at run time; the
 * rest is type-only.
 */
export interface PageDefinition<Data = unknown, Actions = unknown, Params = unknown, Result = unknown, Messages = unknown> {
  readonly data: Data;
  readonly actions: ViewCallable<Actions>;
  readonly __lxPage: { readonly params: Params; readonly result: Result; readonly messages: Messages };
}

/**
 * The Logic end of a channel a View opens with `useChannel`: an action
 * `ticker(params, channel: PageChannel<Tick, Command>)` sends `Tick`s and
 * receives `Command`s.
 */
export interface PageChannel<Send = unknown, Receive = unknown> {
  send(payload: Send): void;
  close(code?: string, reason?: string): void;
  /** Returns a function that removes the handler. */
  on(event: 'data', handler: (payload: Receive) => void): () => void;
  on(event: 'close', handler: (info: { code: string; reason: string }) => void): () => void;
}

/** The params a page definition declares. */
export type ParamsOf<P> = P extends { readonly __lxPage: { readonly params: infer T } } ? T : never;
/** The result a page definition declares. */
export type ResultOf<P> = P extends { readonly __lxPage: { readonly result: infer T } } ? T : never;
/** The messages a page definition declares. */
export type MessagesOf<P> = P extends { readonly __lxPage: { readonly messages: infer T } } ? T : never;

/** What every `definePage` form declares besides its behaviour. */
interface PageDeclarations<Data, Params, Result, Messages> {
  params?: Declared<Params>;
  result?: Declared<Result>;
  messages?: Declared<Messages>;
  data?: Data & JsonData<Data>;
}

/**
 * Define a page. Declare what it takes and returns at the top, then its
 * behaviour in one of two forms. `setup(page)` is the form for lifecycle
 * hooks and per-instance state: it runs once per instance, before `onLoad`,
 * and returns its hooks and `actions`. `actions: page => ({ … })` is the
 * short form for a page with only actions; it also runs once per instance.
 *
 * ```ts
 * export default definePage({
 *   params: type<{ id: string }>(),
 *   data: { order: null as Order | null },
 *   setup(page) {
 *     return {
 *       async onLoad({ id }) { const order = await load(id); page.update(d => { d.order = order; }); },
 *       actions: { close() { return page.close(); } },
 *     };
 *   },
 * });
 *
 * export default definePage({
 *   data: { count: 0 },
 *   actions: page => ({ bump() { page.update(d => { d.count += 1; }); } }),
 * });
 * ```
 */
export function definePage<
  Data extends object = {},
  Params = PageLoadOptions,
  Result = never,
  Messages = never,
  Actions extends object = {},
  R = {},
>(definition: PageDeclarations<Data, Params, Result, Messages> & {
  setup?: (page: PageApi<Data, Params, Result, Messages>) => PageSetup<Params, Actions> & R & ExactSetup<R> & SyncSetup<R>;
  actions?: undefined;
}): PageDefinition<Data, Actions, Params, Result, Messages>;
export function definePage<
  Data extends object = {},
  Params = PageLoadOptions,
  Result = never,
  Messages = never,
  Actions extends object = {},
  S = never,
>(definition: PageDeclarations<Data, Params, Result, Messages> & {
  actions: (page: PageApi<Data, Params, Result, Messages>) => Actions & ThisType<void> & SyncActions<Actions>;
  setup?: S & OnlyOne<S>;
}): PageDefinition<Data, Actions, Params, Result, Messages>;
export function definePage<
  Data extends object = {},
  Params = PageLoadOptions,
  Result = never,
  Messages = never,
  Actions extends object = {},
  S = never,
>(definition: PageDeclarations<Data, Params, Result, Messages> & {
  actions: Actions & ThisType<void> & ActionMap<Actions>;
  setup?: S & OnlyOne<S>;
}): PageDefinition<Data, Actions, Params, Result, Messages>;
export function definePage(definition: { setup?: unknown; actions?: unknown }): unknown {
  if (definition === null || typeof definition !== 'object') {
    throw new TypeError('definePage takes { params?, result?, messages?, data?, setup? or actions? }');
  }
  if (definition.setup !== undefined && definition.actions !== undefined) {
    throw new TypeError('definePage: give setup or actions, not both; with setup, return actions from it');
  }
  Object.defineProperty(definition, Symbol.for('lingxia.page'), { value: true });
  return definition;
}

type SyncActions<A> = A extends PromiseLike<unknown>
  ? { 'LingXia type error': 'actions(page) is synchronous; start async work in an action' }
  : unknown;

type ActionMap<A> = A extends (...args: never[]) => unknown
  ? { 'LingXia type error': 'actions is an object of functions, or page => ({ … }) returning one synchronously' }
  : unknown;

type OnlyOne<S> = [S] extends [never] ? unknown
  : { 'LingXia type error': 'give setup or actions, not both; with setup, return actions from it' };

export type { JsonValue };
