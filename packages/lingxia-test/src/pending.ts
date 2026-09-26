/**
 * Asynchronous work that spec code started in the test context, so a spec
 * that times out without settling can name what it left behind.
 *
 * While a run is active the runtime wraps the test context's `setTimeout`,
 * `setInterval` and `fetch`, and `rawAutomation()` hands out a driver that
 * does the same for its calls; each call made while a spec owns the context is
 * recorded under that spec until it fires, is cleared, or settles. The
 * runner's own timers go through `runnerSetTimeout` / `runnerClearTimeout`,
 * which are never recorded.
 */

export type PendingKind = "timer" | "interval" | "fetch" | "eval" | "action";

export interface PendingWork {
  kind: PendingKind;
  /** What it is: `setTimeout 60000ms`, `GET https://…`, `t.app.logic.eval`. */
  detail: string;
  /** The spec that started it (its full name). */
  owner: string;
  /** Milliseconds since the owning spec started, when it was started. */
  at_ms?: number;
}

type TimerFn = (callback: (...args: unknown[]) => unknown, ms?: number, ...args: unknown[]) => unknown;
type ClearFn = (handle: unknown) => void;
type FetchFn = (input: unknown, init?: { method?: string }) => Promise<unknown>;

interface Natives {
  setTimeout: TimerFn;
  clearTimeout: ClearFn;
  setInterval: TimerFn;
  clearInterval: ClearFn;
  fetch?: FetchFn;
}

const scope = globalThis as unknown as Record<string, unknown>;

function captureNatives(): Natives {
  return {
    setTimeout: scope.setTimeout as TimerFn,
    clearTimeout: scope.clearTimeout as ClearFn,
    setInterval: scope.setInterval as TimerFn,
    clearInterval: scope.clearInterval as ClearFn,
    fetch: typeof scope.fetch === "function" ? (scope.fetch as FetchFn) : undefined,
  };
}

let natives: Natives = captureNatives();
let installed: Natives | undefined;
let owner: { name: string; started: number } | undefined;
const timers = new Map<unknown, PendingWork & { kind: "timer" | "interval" }>();
const fetches = new Set<PendingWork>();
const rawCalls = new Set<PendingWork>();

/** A runner timer: never recorded as spec work, never cancelled with it. */
export function runnerSetTimeout(callback: () => void, ms: number): unknown {
  return (installed ?? natives).setTimeout.call(globalThis, callback, ms);
}

export function runnerClearTimeout(handle: unknown): void {
  (installed ?? natives).clearTimeout.call(globalThis, handle);
}

function entry<K extends PendingKind>(kind: K, detail: string): PendingWork & { kind: K } | undefined {
  if (!owner) return undefined;
  return { kind, detail, owner: owner.name, at_ms: Date.now() - owner.started };
}

function describeRequest(input: unknown, init?: { method?: string }): string {
  const request = input as { url?: unknown; method?: unknown } | undefined;
  const url = typeof input === "string" ? input : typeof request?.url === "string" ? request.url : String(input);
  const method = init?.method ?? (typeof request?.method === "string" ? request.method : "GET");
  return `${String(method).toUpperCase()} ${url.length > 120 ? `${url.slice(0, 117)}...` : url}`;
}

/** Start recording spec work in the test context. Idempotent. */
export function installPendingTracker(): void {
  if (installed) return;
  natives = captureNatives();
  const base = natives;
  installed = base;
  const trackedTimeout: TimerFn = function (callback, ms, ...args) {
    const work = typeof callback === "function" ? entry("timer", `setTimeout ${Number(ms ?? 0)}ms`) : undefined;
    if (!work) return base.setTimeout.call(globalThis, callback, ms, ...args);
    let handle: unknown;
    handle = base.setTimeout.call(globalThis, (...fired: unknown[]) => {
      timers.delete(handle);
      return callback(...fired);
    }, ms, ...args);
    timers.set(handle, work);
    return handle;
  };
  const trackedInterval: TimerFn = function (callback, ms, ...args) {
    const handle = base.setInterval.call(globalThis, callback, ms, ...args);
    const work = typeof callback === "function" ? entry("interval", `setInterval ${Number(ms ?? 0)}ms`) : undefined;
    if (work) timers.set(handle, work);
    return handle;
  };
  const clear = (native: ClearFn): ClearFn => function (handle) {
    timers.delete(handle);
    native.call(globalThis, handle);
  };
  scope.setTimeout = trackedTimeout;
  scope.setInterval = trackedInterval;
  scope.clearTimeout = clear(base.clearTimeout);
  scope.clearInterval = clear(base.clearInterval);
  if (base.fetch) {
    const nativeFetch = base.fetch;
    scope.fetch = function (input: unknown, init?: { method?: string }) {
      const call = nativeFetch.call(globalThis, input, init);
      const work = entry("fetch", describeRequest(input, init));
      if (work) {
        fetches.add(work);
        const done = () => { fetches.delete(work); };
        void Promise.resolve(call).then(done, done);
      }
      return call;
    };
  }
}

/**
 * `root` with every promise-returning call recorded as pending work of the
 * spec that made it (`eval` calls as `eval`), so a raw driver call a
 * timed-out spec still awaits can be named. Getters and methods run against
 * the original object, so native receivers keep working.
 */
export function trackDriver<T extends object>(root: T, path: string): T {
  return new Proxy(root, {
    get(target, prop) {
      const value: unknown = Reflect.get(target, prop, target);
      if (typeof prop !== "string") return value;
      if (typeof value === "function") {
        return (...args: unknown[]) => {
          const result: unknown = (value as (...input: unknown[]) => unknown).apply(target, args);
          const name = `${path}${prop}`;
          if (result && typeof (result as { then?: unknown }).then === "function") {
            const work = entry(prop === "eval" ? "eval" : "action", `${name}()`);
            if (work) {
              rawCalls.add(work);
              const done = () => { rawCalls.delete(work); };
              void Promise.resolve(result).then(done, done);
            }
            return result;
          }
          return result && typeof result === "object" ? trackDriver(result, `${name}().`) : result;
        };
      }
      return value && typeof value === "object" ? trackDriver(value, `${path}${prop}.`) : value;
    },
  });
}

/**
 * The automation root with its lxapp drivers tracked (`trackDriver`). The
 * host tiers (`lxapps`, `browser`, `shell`, `device`, `desktop`,
 * `terminal`) are handed out as the native objects: the Rong host objects
 * behind them lose their methods when read through a proxy.
 */
export function trackAutomationRoot<T extends object>(root: T): T {
  return new Proxy(root, {
    get(target, prop) {
      const value: unknown = Reflect.get(target, prop, target);
      if (typeof value !== "function") return value;
      if (prop !== "lxapp") return (value as (...input: unknown[]) => unknown).bind(target);
      return (...args: unknown[]) => {
        const driver: unknown = (value as (...input: unknown[]) => unknown).apply(target, args);
        const path = `rawAutomation().lxapp(${args.length > 0 ? JSON.stringify(args[0]) : ""}).`;
        return driver && typeof driver === "object" ? trackDriver(driver, path) : driver;
      };
    },
  });
}

/** Put the test context's own timers and `fetch` back. */
export function uninstallPendingTracker(): void {
  if (!installed) return;
  scope.setTimeout = installed.setTimeout;
  scope.clearTimeout = installed.clearTimeout;
  scope.setInterval = installed.setInterval;
  scope.clearInterval = installed.clearInterval;
  if (installed.fetch) scope.fetch = installed.fetch;
  installed = undefined;
  owner = undefined;
  timers.clear();
  fetches.clear();
  rawCalls.clear();
}

/** The spec whose code runs from now on, or `undefined` between specs. */
export function setPendingOwner(name: string | undefined): void {
  owner = name === undefined ? undefined : { name, started: Date.now() };
}

/** Timers, fetches and raw driver calls `name` started that have not fired, been cleared or settled. */
export function pendingOf(name: string): PendingWork[] {
  return [...rawCalls, ...fetches, ...timers.values()].filter((work) => work.owner === name);
}

/**
 * Clear the timers and intervals `name` left: a timed-out spec is abandoned,
 * and its polls must not fire into the specs after it. Returns how many.
 */
export function cancelTimersOf(name: string): number {
  let cancelled = 0;
  for (const [handle, work] of [...timers]) {
    if (work.owner !== name) continue;
    timers.delete(handle);
    const native = installed ?? natives;
    (work.kind === "interval" ? native.clearInterval : native.clearTimeout).call(globalThis, handle);
    cancelled += 1;
  }
  return cancelled;
}

/** `timer setTimeout 60000ms (at 12ms), fetch GET https://…` */
export function describePending(work: readonly PendingWork[]): string {
  if (work.length === 0) {
    return "an awaited promise that no timer, fetch or fixture call of the spec backs (e.g. `new Promise(() => {})`, or a raw driver call)";
  }
  return work
    .map((item) => `${item.kind} ${item.detail}${item.at_ms !== undefined ? ` (started ${item.at_ms}ms into the spec)` : ""}`)
    .join(", ");
}
