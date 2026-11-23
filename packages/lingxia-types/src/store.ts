import type { DeepReadonly, JsonData, Live, SignalOptions } from './generated/logic.js';

/** Shared Logic state: one value for every page of this lxapp. */
export interface Store<T> extends Live<DeepReadonly<T>> {
  /** Replace the value; any `T`, including `null`. Throws inside a `page.update` or `store.update` recipe. */
  set(value: DeepReadonly<T> & JsonData<DeepReadonly<T>>): void;
  /**
   * Change the value in place: `store.update(d => { d.items.push(x); })`. Only for an object or
   * array value; a nullable store uses `set`. Same reentry rule as `set`.
   */
  update<R>(recipe: [T] extends [object] ? StoreRecipe<T, R> : never): void;
  /** A read-only part of it; changes only when the picked value does (deep equal), so pick small values. */
  pick<U>(select: (value: DeepReadonly<T>) => U): Live<U>;
}

type StoreRecipe<T, R> = (draft: T) => R & StoreRecipeReturn<T, R>;

type StoreRecipeReturn<T, R> = R extends PromiseLike<unknown>
  ? { 'LingXia type error': 'a recipe is synchronous; await first, then update' }
  : R extends readonly unknown[] ? unknown
  : R extends object
    ? R extends DeepReadonly<T> ? { 'LingXia type error': 'change the draft in place, or replace the value with store.set' } : unknown
    : unknown;

/** `defineStore`'s options, or a missing-argument error when the value is not JSON data. */
// `initial: T` alone keeps `defineStore(false)` a `Store<boolean>`.
type JsonGuard<T> = [T] extends [JsonData<T>] ? [options?: StoreOptions] : [store_value_must_be_JSON_data: never];

/** How a store is made. */
export interface StoreOptions {
  /** Names the store in errors and dev logs; defaults to `persist.key`, else the place it was defined. */
  name?: string;
}

/** A persisted store's last load or save. Never a rejection. */
export type PersistResult =
  | {
      /** It worked, or there was nothing stored. */
      ok: true;
    }
  | {
      /** It failed; the value in memory is unchanged by it. */
      ok: false;
      /** What `load` or `save` threw. */
      error: unknown;
    };

/**
 * Where a store keeps its value: one `lx.storage` key, or the app's own `load` and `save`
 * (layout, migration and old keys live in `load`). `load` resolving `undefined` means empty.
 */
export type StorePersist<T> =
  | {
      /** The `lx.storage` key holding the value. */
      key: string;
    }
  | { load(): Promise<T | undefined>; save(value: DeepReadonly<T>): Promise<void> };

/** How a persisted store is made. */
export interface PersistedStoreOptions<T> extends StoreOptions {
  /** Load once at definition, save after each change. */
  persist: StorePersist<T>;
}

/** A store that loads its value at definition and saves each change. */
export interface PersistedStore<T> extends Store<T> {
  /**
   * Settles when the first load does. Empty or failed keeps `initial`; a change made before it
   * settles wins over the loaded value.
   */
  readonly ready: Promise<PersistResult>;
  /** Load again, as `ready` did. */
  reload(): Promise<PersistResult>;
  /** The last finished save. Saves run one at a time, each with the newest value. */
  readonly saved: Live<PersistResult>;
}

/** `defineStore`'s persisted options, or a missing-argument error when the value is not JSON data. */
// NoInfer: `T` comes from `initial` alone, so `defineStore(0, { persist })` stays a number store.
type PersistGuard<T> = [T] extends [JsonData<T>] ? [options: PersistedStoreOptions<NoInfer<T>>] : [store_value_must_be_JSON_data: never];

type Persistence<T> = {
  load(): Promise<T | undefined>;
  save(value: DeepReadonly<T>): Promise<void>;
};

type StorageLx = { storage: { get(key: string): Promise<unknown>; set(key: string, value: unknown): Promise<void> } };

function persistence<T>(persist: StorePersist<T>, label: string): Persistence<T> {
  if (persist === null || typeof persist !== 'object') {
    throw new TypeError(`${label}: persist is { key } or { load, save }`);
  }
  if ('key' in persist) {
    if (typeof persist.key !== 'string' || persist.key === '') throw new TypeError(`${label}: persist.key is a storage key`);
    const key = persist.key;
    // Read at each call: a module store is defined before a test swaps `lx`.
    const storage = () => (globalThis as { lx?: StorageLx }).lx!.storage;
    return {
      load: () => storage().get(key) as Promise<T | undefined>,
      save: (value) => storage().set(key, value),
    };
  }
  if (typeof persist.load !== 'function' || typeof persist.save !== 'function') {
    throw new TypeError(`${label}: persist is { key } or { load, save }`);
  }
  return persist;
}

// Shared with the page runtime: a store write inside any recipe would run behind the recipe's back.
const RECIPE = Symbol.for('lingxia.recipeDepth');
type RecipeDepth = { [RECIPE]?: number };

function inRecipe(): boolean {
  return ((globalThis as RecipeDepth)[RECIPE] ?? 0) > 0;
}

function withinRecipe<R>(run: () => R): R {
  const scope = globalThis as RecipeDepth;
  scope[RECIPE] = (scope[RECIPE] ?? 0) + 1;
  try {
    return run();
  } finally {
    scope[RECIPE] = (scope[RECIPE] ?? 1) - 1;
  }
}

/** A deep, frozen copy of JSON data; throws naming the first part that is not JSON. */
function frozenJson(value: unknown, where: string, seen: Set<object> = new Set()): unknown {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value;
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new TypeError(`${where} is ${value}; a store holds JSON data`);
    return value;
  }
  if (typeof value !== 'object') throw new TypeError(`${where} is a ${typeof value}; a store holds JSON data`);
  if (seen.has(value)) throw new TypeError(`${where} refers back to itself; a store holds JSON data`);
  seen.add(value);
  let copy: unknown;
  if (Array.isArray(value)) {
    copy = value.map((item, index) => frozenJson(item, `${where}[${index}]`, seen));
  } else {
    const prototype = Object.getPrototypeOf(value);
    if (prototype !== Object.prototype && prototype !== null) {
      throw new TypeError(`${where} is a ${prototype?.constructor?.name ?? 'class instance'}; a store holds JSON data`);
    }
    const out: Record<string, unknown> = {};
    for (const [key, item] of Object.entries(value)) {
      if (item === undefined) continue;
      out[key] = frozenJson(item, `${where}.${key}`, seen);
    }
    copy = out;
  }
  seen.delete(value);
  return Object.freeze(copy);
}

/** A writable copy of frozen JSON, for a recipe's draft. */
function thawed<T>(value: T): T {
  if (value === null || typeof value !== 'object') return value;
  if (Array.isArray(value)) return value.map(thawed) as T;
  const out: Record<string, unknown> = {};
  for (const [key, item] of Object.entries(value)) out[key] = thawed(item);
  return out as T;
}

function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (a === null || b === null || typeof a !== 'object' || typeof b !== 'object') return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  const keys = Object.keys(a);
  if (keys.length !== Object.keys(b).length) return false;
  return keys.every((key) => Object.prototype.hasOwnProperty.call(b, key)
    && deepEqual((a as Record<string, unknown>)[key], (b as Record<string, unknown>)[key]));
}

/** Watchers of one value: the first `watch` call runs at once, a signal ends one. */
class Watchers<V> {
  private readonly callbacks = new Set<(value: V) => void>();

  constructor(private readonly onFirst?: () => void, private readonly onLast?: () => void) {}

  get size(): number {
    return this.callbacks.size;
  }

  add(current: () => V, callback: (value: V) => void, options?: SignalOptions): () => void {
    if (typeof callback !== 'function') throw new TypeError('watch takes a callback: watch(value => { … })');
    const signal = options?.signal;
    if (signal?.aborted) return () => {};
    // Each watch is its own entry, so the same function may follow twice.
    const entry = (value: V) => callback(value);
    if (this.callbacks.size === 0) this.onFirst?.();
    this.callbacks.add(entry);
    const stop = () => {
      if (!this.callbacks.delete(entry)) return;
      signal?.removeEventListener('abort', stop);
      if (this.callbacks.size === 0) this.onLast?.();
    };
    signal?.addEventListener('abort', stop, { once: true });
    callback(current());
    return stop;
  }

  notify(value: V, label: string): void {
    for (const callback of [...this.callbacks]) {
      try {
        callback(value);
      } catch (error) {
        reportStoreFailure(`${label}: a watcher threw`, error);
      }
    }
  }
}

type FailureReporter = (failure: { error: unknown; source: { kind: 'app'; hook: 'store' } }, page: null) => void;

// Through `App.onError` when the runtime has one; logged otherwise.
function reportStoreFailure(message: string, error: unknown): void {
  const report = (globalThis as { __LX_REPORT_FAILURE__?: FailureReporter }).__LX_REPORT_FAILURE__;
  if (typeof report === 'function') report({ error, source: { kind: 'app', hook: 'store' } }, null);
  else console.error(message, error);
}

function definedAt(): string {
  const line = new Error().stack?.split('\n').find((frame, index) => index > 2 && !frame.includes('defineStore'));
  const place = line?.trim().replace(/^at\s+/, '').replace(/^.*\((.*)\)$/, '$1');
  return place ? `store at ${place}` : 'store';
}

/**
 * Shared Logic state: one value for every page of this lxapp, defined once at module scope.
 * Its value is JSON data, as page data is.
 *
 * ```ts
 * export const cart = defineStore({ items: [] as Item[] });
 * cart.update(d => { d.items.push(item); });
 * const count = cart.pick(c => c.items.length);
 * ```
 */
export function defineStore<T>(initial: T, ...rest: PersistGuard<T>): PersistedStore<T>;
export function defineStore<T>(initial: T, ...rest: JsonGuard<T>): Store<T>;
export function defineStore<T>(initial: T, ...rest: unknown[]): Store<T> | PersistedStore<T> {
  const options = rest[0] as (StoreOptions & { persist?: StorePersist<T> }) | undefined;
  if (options !== undefined && (options === null || typeof options !== 'object')) {
    throw new TypeError('defineStore takes (initial, { name?, persist? })');
  }
  const persistKey = options?.persist && 'key' in options.persist ? options.persist.key : undefined;
  const label = options?.name ?? persistKey ?? definedAt();
  const adapter = options?.persist !== undefined ? persistence(options.persist, label) : undefined;
  let value = frozenJson(initial, `${label}: the initial value`) as DeepReadonly<T>;
  const watchers = new Watchers<DeepReadonly<T>>();
  // Counts changes made here, so a load that finishes after one is dropped.
  let changes = 0;
  let loaded = adapter === undefined;
  let warnedEarlyRead = false;

  const replace = (next: DeepReadonly<T>, fromLoad = false) => {
    if (deepEqual(next, value)) return;
    value = next;
    if (!fromLoad) {
      changes += 1;
      scheduleSave();
    }
    watchers.notify(value, label);
  };

  // One save in flight at most; a change meanwhile marks it dirty and the next save takes the
  // newest value, so an old save never lands after a newer one.
  let saving = false;
  let dirty = false;
  let savedValue: PersistResult = { ok: true };
  const savedWatchers = new Watchers<PersistResult>();
  function scheduleSave() {
    if (!adapter) return;
    dirty = true;
    if (!saving) {
      saving = true;
      void Promise.resolve().then(runSave);
    }
  }
  function runSave() {
    dirty = false;
    const snapshot = value;
    Promise.resolve()
      .then(() => adapter!.save(snapshot))
      .then(
        (): PersistResult => ({ ok: true }),
        (error: unknown): PersistResult => {
          console.warn(`${label}: saving failed`, error);
          return { ok: false, error };
        },
      )
      .then((result) => {
        savedValue = result;
        savedWatchers.notify(result, label);
        if (dirty) runSave();
        else saving = false;
      });
  }
  function load(): Promise<PersistResult> {
    const before = changes;
    return Promise.resolve()
      .then(() => adapter!.load())
      .then((stored): PersistResult => {
        if (stored !== undefined && changes === before) {
          replace(frozenJson(stored, `${label}: the loaded value`) as DeepReadonly<T>, true);
        }
        return { ok: true };
      })
      .then((result) => result, (error: unknown): PersistResult => ({ ok: false, error }));
  }
  const refuseInRecipe = (method: string) => {
    if (inRecipe()) throw new Error(`${label}: store.${method} cannot be called inside a page.update or store.update recipe`);
  };

  const store: Store<T> = {
    get: () => {
      if (!loaded && !warnedEarlyRead) {
        warnedEarlyRead = true;
        console.warn(`${label}: read before its first load settled; await ready, or follow it from a page`);
      }
      return value;
    },
    watch: (callback, watchOptions) => watchers.add(() => value, callback, watchOptions),
    set(next) {
      refuseInRecipe('set');
      replace(frozenJson(next, `${label}: store.set`) as DeepReadonly<T>);
    },
    update(recipe: (draft: T) => unknown) {
      if (typeof recipe !== 'function') {
        throw new TypeError(`${label}: store.update takes a recipe: store.update(d => { d.n += 1; })`);
      }
      refuseInRecipe('update');
      if (value === null || typeof value !== 'object') {
        throw new TypeError(`${label}: store.update needs an object or array value; replace a ${value === null ? 'null' : typeof value} with store.set`);
      }
      const draft = thawed(value) as T;
      const returned = withinRecipe(() => recipe(draft));
      if (returned !== null && typeof returned === 'object') {
        if (typeof (returned as PromiseLike<unknown>).then === 'function') {
          throw new TypeError(`${label}: store.update: the recipe must be synchronous; await first, then update`);
        }
        if (!Array.isArray(returned) && returned !== draft) {
          throw new TypeError(`${label}: store.update: change the draft in place, or replace the value with store.set`);
        }
      }
      replace(frozenJson(draft, `${label}: store.update`) as DeepReadonly<T>);
    },
    pick<U>(select: (current: DeepReadonly<T>) => U): Live<U> {
      if (typeof select !== 'function') throw new TypeError(`${label}: store.pick takes a function: store.pick(s => s.count)`);
      let picked: U;
      let stopSource: (() => void) | undefined;
      // Unwatched, a pick holds no subscription; `get` reads the store afresh.
      const pickedWatchers: Watchers<U> = new Watchers<U>(
        () => {
          picked = select(value);
          let first = true;
          stopSource = watchers.add(() => value, (current) => {
            if (first) { first = false; return; }
            const next = select(current);
            if (deepEqual(next, picked)) return;
            picked = next;
            pickedWatchers.notify(picked, label);
          });
        },
        () => {
          stopSource?.();
          stopSource = undefined;
        },
      );
      return {
        get: () => (stopSource ? picked : select(value)),
        watch: (callback, watchOptions) => pickedWatchers.add(() => picked, callback, watchOptions),
      };
    },
  };
  if (!adapter) return Object.freeze(store);
  const ready = load().then((result) => {
    loaded = true;
    return result;
  });
  return Object.freeze({
    ...store,
    ready,
    reload: load,
    saved: Object.freeze({
      get: () => savedValue,
      watch: (callback: (result: PersistResult) => void, watchOptions?: SignalOptions) =>
        savedWatchers.add(() => savedValue, callback, watchOptions),
    }),
  });
}

type LiveValues<Sources> = { [K in keyof Sources]: Sources[K] extends Live<infer V> ? V : never };

/**
 * A read-only `Live` computed from others (stores, picks, host `Live`s). It recomputes when a
 * source changes and notifies only on a deep-unequal result; it holds its sources only while watched.
 *
 * ```ts
 * export const language = derive([override, lx.app.language], (o, l) => o ?? l);
 * ```
 */
export function derive<const Sources extends readonly Live<unknown>[], U>(
  sources: Sources,
  compute: (...values: LiveValues<Sources>) => U,
): Live<U> {
  if (!Array.isArray(sources) || sources.some((source) => !source || typeof source.get !== 'function' || typeof source.watch !== 'function')) {
    throw new TypeError('derive takes an array of Live values and a function: derive([a, b], (a, b) => …)');
  }
  if (typeof compute !== 'function') throw new TypeError('derive takes a function: derive([a, b], (a, b) => …)');
  const run = (values: unknown[]) => compute(...(values as LiveValues<Sources>));
  let values: unknown[] = [];
  let result: U;
  let stops: (() => void)[] = [];
  const watchers: Watchers<U> = new Watchers<U>(
    () => {
      values = sources.map((source) => source.get());
      result = run(values);
      let ready = false;
      stops = sources.map((source, index) => source.watch((value) => {
        values[index] = value;
        if (!ready) return;
        let next: U;
        try {
          next = run(values);
        } catch (error) {
          reportStoreFailure('derive: the compute function threw', error);
          return;
        }
        if (deepEqual(next, result)) return;
        result = next;
        watchers.notify(result, 'derive');
      }));
      ready = true;
    },
    () => {
      for (const stop of stops) stop();
      stops = [];
    },
  );
  return Object.freeze({
    get: () => (stops.length > 0 ? result : run(sources.map((source) => source.get()))),
    watch: (callback: (value: U) => void, options?: SignalOptions) => watchers.add(() => result, callback, options),
  });
}
