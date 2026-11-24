
import type {
  DeepReadonly,
  JsonData,
  JsonValue,
  NavigationBarPatch,
  PageLoadOptions,
  Live,
  PageEnvContext,
  MessageIn,
  MessageOut,
  PageSurface,
  SignalOptions,
  PagePlayer,
  DeviceOrientation,
} from './generated/logic.js';

export type { JsonData } from './generated/logic.js';
export { defineStore, derive } from './store.js';
export { dateParts, fromDateParts, isTimeZone } from './time.js';
export type { DateParts, TimeZoneOptions, WallTime, Weekday } from './time.js';
export type { PersistedStore, PersistedStoreOptions, PersistResult, Store, StoreOptions, StorePersist } from './store.js';

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
export type PageHookName = 'onLoad' | 'onShow' | 'onReady' | 'onHide' | 'onUnload' | 'onRefresh';

type Hook = () => void | Promise<void>;

/** What `onShow` receives. */
export interface ShowOptions<Params> {
  /**
   * The params this showing arrived with: an open, a tab switch or a reuse that carried params
   * (also when the page was already on screen); absent on every other show.
   */
  readonly params?: Params;
}

/** What `setup` returns: lifecycle hooks under their names, and `actions`. */
export interface PageSetup<Params, Actions> {
  /** Runs once with the params the page was opened with. */
  onLoad?: (params: Params) => void | Promise<void>;
  /** Runs each time the page comes on screen. */
  onShow?: (options: ShowOptions<Params>) => void | Promise<void>;
  /** Runs once, after the View first rendered. */
  onReady?: Hook;
  /** Runs each time the page leaves the screen but stays alive. */
  onHide?: Hook;
  /** Runs once as the page goes away, after `page.signal` aborts. */
  onUnload?: Hook;
  /** A pull, or `page.refresh()`. The indicator stays until the returned promise settles. */
  onRefresh?: Hook;
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

type SyncReturn<R> = R extends PromiseLike<unknown>
  ? { 'LingXia type error': 'a recipe is synchronous; await first, then update' }
  : unknown;

// A key's read-only form is accepted: `set` replaces whole keys, so `page.data` values can go back in.
/** The keys `page.set` assigns, each as JSON data in its mutable or read-only form. */
type SetPatch<Data> = Partial<DeepReadonly<Data>> & JsonData<Partial<DeepReadonly<Data>>>;

type RecipeReturn<R, Data> = R extends PromiseLike<unknown>
  ? { 'LingXia type error': 'a recipe is synchronous; await first, then update' }
  : R extends readonly unknown[] ? unknown
  : R extends object
    ? R extends DeepReadonly<Data> ? { 'LingXia type error': 'change the draft in place; a returned object is not applied' } : unknown
    : unknown;

/** What a helper in `logic/` needs of a page to read and change its data: `PageApi` without the rest. */
export interface PageLike<Data> {
  /** The page's current data. */
  readonly data: DeepReadonly<Data>;
  /** Change data through a draft, as `page.update` does; the recipe must be synchronous. */
  // Only the synchronous check: one that compares the return with `Data` would refuse a page with more keys.
  update<R>(recipe: (draft: Data) => R & SyncReturn<R>): void;
  /** Assign whole keys, as `page.set` does. */
  set(patch: SetPatch<Data>): void;
}

/** One page instance, as `setup` and everything it returns see it. */
export interface PageApi<Data, Params, Result, Messages = never, Follow = {}> {
  /** The current data, with the followed keys. Readonly: change it with `update`. */
  readonly data: DeepReadonly<WithFollowed<Data, Follow>>;
  /**
   * The one way to change data: `page.update(d => { d.n += 1; })`. `page.data`
   * changes at once; the View gets the changes once per turn. Throws inside
   * another `update`; ignored after the page unloads.
   */
  update<R>(recipe: UpdateRecipe<Data, R>): void;
  /** Assign these keys, as `update` would: `page.set({ loading: false })`. Same rules as `update`. */
  set(patch: SetPatch<Data>): void;
  /**
   * Resolves when the View has everything updated so far. Rejects on unload.
   * Throws while `setup` runs; call it from a hook or an action.
   */
  flush(): Promise<void>;
  /** This instance's params: what it opened with, replaced by each showing that carries params. */
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
    /** Patch it; unset fields stay as they are. Rejects `E_INVALID_STATE` (`'pageUnloaded'`) once the page has unloaded. */
    update(patch: NavigationBarPatch): Promise<void>;
  };
  /**
   * Show the refresh indicator and run `onRefresh`, as a pull would.
   * Throws `E_INVALID_STATE` with `reason` `'refreshNotEnabled'` unless the page config sets
   * `enablePullDownRefresh`, or `'pageUnloaded'` once the page has unloaded.
   */
  refresh(): void;
  /** The player named `name` on this page: a `useVideo` name or `<video id>`, or an `<lx-video>` id. */
  video(name: string): PagePlayer;
  /**
   * Lock the window to `portrait` or `landscape` while this page is current.
   * The lock beats `pageOrientation`; only a full-screen video beats it. It
   * ends when the page unloads, so the next page gets its own orientation.
   * `lock` throws `E_INVALID_ARG` for any other value.
   */
  readonly orientation: {
    /** Place the lock; replaces one already placed. */
    lock(orientation: DeviceOrientation): void;
    /** Remove the lock: `pageOrientation`, then the app's default, applies again. */
    unlock(): void;
  };
  /**
   * The size class this page lays out for. `watch` delivers it at once, then
   * when `sizeClass` or `canDock` changes (not on every resize), and stops
   * when the page unloads.
   */
  readonly env: Live<PageEnvContext>;
  /**
   * Whether the page is on screen: `onShow` and `onHide`, as a value. Already `true` as `setup`
   * runs for a page that opens showing; `false` for a tab preloaded behind another.
   */
  readonly visible: Live<boolean>;
  /**
   * Run `fn` once after `ms`, shown or not; returns `stop()`. Stops when the page unloads, and does
   * not start after. A throw or rejection is reported as an unhandled failure.
   */
  after(ms: number, fn: () => void | Promise<void>): () => void;
  /**
   * Run `fn` every `ms`, counted from the end of each run, so a slow run never piles up; returns
   * `stop()`. Paused while the page is hidden, and a run missed meanwhile happens when it shows
   * again. A throw or rejection is reported and the timer goes on. Stops when the page unloads.
   */
  every(ms: number, fn: () => void | Promise<void>, options?: PageEveryOptions): () => void;
}

/** How `page.every` behaves while the page is hidden. */
export interface PageEveryOptions {
  /**
   * `'run'` keeps it running behind another page or in the background; for the rare timer that must.
   * @default 'pause'
   */
  whenHidden?: 'pause' | 'run';
}

/** Page actions as the View calls them: each resolves to its result. */
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
  /** The page's initial data. */
  readonly data: Data;
  /** The page's actions, as the View calls them. */
  readonly actions: ViewCallable<Actions>;
  /** What the page declares, for the type system only. */
  readonly __lxPage: {
    /** What it is opened with. */
    readonly params: Params;
    /** What it closes with. */
    readonly result: Result;
    /** What it sends its opener. */
    readonly messages: Messages;
    /** The actions as written, which `mount` calls directly. */
    readonly actions: Actions;
  };
}

/**
 * The Logic end of a channel a View opens with `useChannel`: an action
 * `ticker(params, channel: PageChannel<Tick, Command>)` sends `Tick`s and
 * receives `Command`s.
 */
export interface PageChannel<Send = unknown, Receive = unknown> {
  /** Send one payload to the View. */
  send(payload: Send): void;
  /** End the channel; the View sees `code` and `reason`. */
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
interface PageDeclarations<Data, Params, Result, Messages, Follow> {
  /** What the page is opened with: `type<{ id: string }>()`. */
  params?: Declared<Params>;
  /** What the page closes with. */
  result?: Declared<Result>;
  /** What the page sends its opener. */
  messages?: Declared<Messages>;
  /** The initial data, JSON only. */
  data?: Data & JsonData<Data>;
  /**
   * Data keys that show a `Live` (a store, `store.pick(…)`, a host `Live`), current from the first
   * frame until the page unloads: `follow: { plan: session.pick(s => s.plan) }`. Read-only in
   * `update` and `set`; change the source instead.
   */
  // NoInfer: without it, a page with no `data` infers its `Data` from `follow`'s keys.
  follow?: Follow & FollowKeysFree<NoInfer<Data>, Follow>;
}

/** What `follow` takes: data key to `Live`. */
type FollowMap = Record<string, Live<unknown>>;

/** A followed key that is also in `data`, as a type error. */
type FollowKeysFree<Data, Follow> = {
  [K in keyof Follow & keyof Data]: { 'LingXia type error': 'a followed key cannot also be in data' };
};

/** The data keys a page's `follow` adds, each the value its `Live` holds. */
type FollowedData<Follow> = { [K in keyof Follow]: Follow[K] extends Live<infer V> ? V : never };

/** A page's data with its followed keys. */
type WithFollowed<Data, Follow> = [keyof Follow] extends [never] ? Data : Data & FollowedData<Follow>;

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
  Follow extends FollowMap = {},
>(definition: PageDeclarations<Data, Params, Result, Messages, Follow> & {
  /** Runs once per instance, before `onLoad`, and returns its hooks and `actions`. */
  setup?: (page: PageApi<Data, Params, Result, Messages, Follow>) => PageSetup<Params, Actions> & R & ExactSetup<R> & SyncSetup<R>;
  /** Not with `setup`: `setup` returns the actions. */
  actions?: undefined;
}): PageDefinition<WithFollowed<Data, Follow>, Actions, Params, Result, Messages>;
export function definePage<
  Data extends object = {},
  Params = PageLoadOptions,
  Result = never,
  Messages = never,
  Actions extends object = {},
  S = never,
  Follow extends FollowMap = {},
>(definition: PageDeclarations<Data, Params, Result, Messages, Follow> & {
  /** Builds the actions once per instance. */
  actions: (page: PageApi<Data, Params, Result, Messages, Follow>) => Actions & ThisType<void> & SyncActions<Actions>;
  /** Not with `actions`. */
  setup?: S & OnlyOne<S>;
}): PageDefinition<WithFollowed<Data, Follow>, Actions, Params, Result, Messages>;
export function definePage<
  Data extends object = {},
  Params = PageLoadOptions,
  Result = never,
  Messages = never,
  Actions extends object = {},
  S = never,
  Follow extends FollowMap = {},
>(definition: PageDeclarations<Data, Params, Result, Messages, Follow> & {
  /** The actions as a plain object, when they need no `page`. */
  actions: Actions & ThisType<void> & ActionMap<Actions>;
  /** Not with `actions`. */
  setup?: S & OnlyOne<S>;
}): PageDefinition<WithFollowed<Data, Follow>, Actions, Params, Result, Messages>;
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
