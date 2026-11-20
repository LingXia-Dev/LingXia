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
  PageMessagePort,
  PageSurface,
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
  /** The page's whole View-callable surface. */
  actions?: Actions;
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

/** A recipe changes the draft in place and returns nothing. */
export type UpdateRecipe<Data> = (draft: Data) => undefined;

/** One page instance, as `setup` and everything it returns see it. */
export interface PageApi<Data, Params, Result> {
  /** The current data. Readonly: change it with `update`. */
  readonly data: DeepReadonly<Data>;
  /**
   * The one way to change data: `page.update(d => { d.n += 1; })`. `page.data`
   * changes at once; the View gets the changes once per turn. Throws inside
   * another `update`; ignored after the page unloads.
   */
  update(recipe: UpdateRecipe<Data>): void;
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
   * The page that opened this one by navigation; absent for a root, a tab,
   * after a restart, and while `setup` runs (read it in a hook or an action).
   */
  readonly opener?: PageMessagePort;
  /** This page's surface when it was opened by `lx.surface.openPage`; absent while `setup` runs. */
  readonly surface?: PageSurface;
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
   * The size class this page lays out for. `watch` runs when `sizeClass` or
   * `canDock` changes, not on every resize, and stops when the page unloads.
   */
  readonly env: PageEnvContext & {
    watch(callback: (env: PageEnvContext) => void): () => void;
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

/** The params a page definition declares. */
export type ParamsOf<P> = P extends { readonly __lxPage: { readonly params: infer T } } ? T : never;
/** The result a page definition declares. */
export type ResultOf<P> = P extends { readonly __lxPage: { readonly result: infer T } } ? T : never;

/**
 * Define a page. Declare what it takes and returns at the top; `setup(page)`
 * runs once per instance, before `onLoad`, and returns its hooks and
 * `actions`, which close over `page` and over per-instance state.
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
 * ```
 */
export function definePage<
  Data extends object = {},
  Params = PageLoadOptions,
  Result = never,
  Messages = never,
  Actions extends object = {},
  R = {},
>(definition: {
  params?: Declared<Params>;
  result?: Declared<Result>;
  messages?: Declared<Messages>;
  data?: Data & JsonData<Data>;
  setup?: (page: PageApi<Data, Params, Result>) => PageSetup<Params, Actions> & R & ExactSetup<R> & SyncSetup<R>;
}): PageDefinition<Data, Actions, Params, Result, Messages> {
  if (definition === null || typeof definition !== 'object') {
    throw new TypeError('definePage takes { params?, result?, messages?, data?, setup? }');
  }
  Object.defineProperty(definition, Symbol.for('lingxia.page'), { value: true });
  return definition as never;
}

export type { JsonValue };
