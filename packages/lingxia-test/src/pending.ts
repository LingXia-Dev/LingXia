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
/** Fetches and raw driver calls in flight, with what settles them. */
const fetches = new Map<PendingWork, Promise<unknown>>();
const rawCalls = new Map<PendingWork, Promise<unknown>>();

/**
 * Raw driver authority handed to spec code, one grant per spec. Every proxy
 * `rawAutomation()` hands out keeps the grant it was made under: a spec the
 * run abandons has its grant revoked, so a handle it still holds refuses its
 * next call while the next spec's own handles keep working. Setup code that
 * runs before the first spec has a grant of its own; between specs there is
 * none, so a continuation of an ended spec cannot take a fresh handle then.
 * Revoking the run (`revokeRawAuthority`) refuses every grant.
 */
export interface Grant {
  revoked?: string;
  /** Host tiers read under this grant: native objects it cannot fence. */
  readonly unfenced: Set<string>;
}

let setupGrant: Grant = { unfenced: new Set() };
let currentGrant: Grant | undefined = setupGrant;
let specsStarted = false;
let revokedReason: string | undefined;

/** Stop spec code driving the app through `rawAutomation()` for the rest of the run. */
export function revokeRawAuthority(reason: string): void {
  revokedReason = reason;
}

/** A new run: `rawAutomation()` works again (earlier proxies stay dead). */
export function restoreRawAuthority(): void {
  revokedReason = undefined;
  if (setupGrant.revoked === undefined) setupGrant.revoked = "it belongs to an earlier run";
  setupGrant = { unfenced: new Set() };
  currentGrant = setupGrant;
  specsStarted = false;
}

/** Why spec code may no longer drive the app, or `undefined`. */
export function rawAuthorityRevoked(): string | undefined {
  return revokedReason;
}

/** The grant of the spec whose code runs now. */
export function currentSpecGrant(): Grant | undefined {
  return currentGrant;
}

/** The spec holding `grant` was abandoned: its handles refuse from now on. */
export function revokeGrant(grant: Grant | undefined, reason: string): void {
  if (grant && grant.revoked === undefined) grant.revoked = reason;
}

function refused(reason: string): Error {
  return Object.assign(new Error(`The run revoked spec code's automation authority: ${reason}`), {
    code: "E_AUTOMATION_PRIVILEGE",
  });
}

function checkGrant(grant: Grant): void {
  if (revokedReason !== undefined) throw refused(revokedReason);
  if (grant.revoked !== undefined) throw refused(`this driver belongs to an abandoned spec: ${grant.revoked}`);
  // A handle setup code took is shared by every spec, so no spec's grant
  // fences it: the spec using it is one the run cannot fence.
  if (grant !== currentGrant && currentGrant !== undefined && currentGrant !== setupGrant) {
    currentGrant.unfenced.add("a rawAutomation() driver taken outside the spec");
  }
}

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
        const done = () => { fetches.delete(work); };
        fetches.set(work, Promise.resolve(call).then(done, done));
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
export function trackDriver<T extends object>(root: T, path: string, grant: Grant = { unfenced: new Set() }): T {
  return new Proxy(root, {
    get(target, prop) {
      const value: unknown = Reflect.get(target, prop, target);
      if (typeof prop !== "string") return value;
      if (typeof value === "function") {
        return (...args: unknown[]) => {
          checkGrant(grant);
          const result: unknown = (value as (...input: unknown[]) => unknown).apply(target, args);
          const name = `${path}${prop}`;
          if (result && typeof (result as { then?: unknown }).then === "function") {
            const work = entry(prop === "eval" ? "eval" : "action", `${name}()`);
            if (work) {
              const done = () => { rawCalls.delete(work); };
              rawCalls.set(work, Promise.resolve(result).then(done, done));
            }
            return result;
          }
          return result && typeof result === "object" ? trackDriver(result, `${name}().`, grant) : result;
        };
      }
      return value && typeof value === "object" ? trackDriver(value, `${path}${prop}.`, grant) : value;
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
  if (revokedReason !== undefined) throw refused(revokedReason);
  const grant = currentGrant;
  if (!grant) {
    throw refused("no spec is running; a spec that already ended cannot take a new driver");
  }
  checkGrant(grant);
  return new Proxy(root, {
    get(target, prop) {
      checkGrant(grant);
      const value: unknown = Reflect.get(target, prop, target);
      if (isAccessor(target, prop) || (value !== null && typeof value === "object")) {
        // A host tier (`shell`, `lxapps`, `desktop`, …) comes from a getter.
        // It is handed out as the native object, whatever `typeof` says (a
        // Rong host object can be callable): wrapping it hides the methods
        // on its prototype. It cannot be fenced here once read, so the run
        // notes it, and stops rather than continue past a spec abandoned
        // while holding one.
        if (value !== null && (typeof value === "object" || typeof value === "function")) {
          grant.unfenced.add(String(prop));
        }
        return value;
      }
      if (typeof value !== "function") return value;
      if (prop !== "lxapp") {
        return (...args: unknown[]) => {
          checkGrant(grant);
          const result: unknown = (value as (...input: unknown[]) => unknown).apply(target, args);
          if (result && typeof result === "object" && typeof (result as { then?: unknown }).then !== "function") {
            grant.unfenced.add(`${String(prop)}()`);
          }
          return result;
        };
      }
      return (...args: unknown[]) => {
        checkGrant(grant);
        const driver: unknown = (value as (...input: unknown[]) => unknown).apply(target, args);
        const path = `rawAutomation().lxapp(${args.length > 0 ? JSON.stringify(args[0]) : ""}).`;
        return driver && typeof driver === "object" ? trackDriver(driver, path, grant) : driver;
      };
    },
  });
}

/** Whether `prop` of `target` (or its prototype chain) is a getter. */
function isAccessor(target: object, prop: PropertyKey): boolean {
  for (let owner: object | null = target; owner; owner = Object.getPrototypeOf(owner) as object | null) {
    const descriptor = Object.getOwnPropertyDescriptor(owner, prop);
    if (descriptor) return typeof descriptor.get === "function";
  }
  return false;
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
export function setPendingOwner(name: string | undefined): Grant | undefined {
  owner = name === undefined ? undefined : { name, started: Date.now() };
  if (name !== undefined) {
    specsStarted = true;
    currentGrant = { unfenced: new Set() };
  } else {
    currentGrant = specsStarted ? undefined : setupGrant;
  }
  return currentGrant;
}

/** Timers, fetches and raw driver calls `name` started that have not fired, been cleared or settled. */
export function pendingOf(name: string): PendingWork[] {
  return [...rawCalls.keys(), ...fetches.keys(), ...timers.values()].filter((work) => work.owner === name);
}

/**
 * Wait up to `ms` for the fetches and raw driver calls `name` left in flight
 * to settle, including ones their continuations start meanwhile. Resolves
 * what is still in flight (empty when all settled).
 */
export async function settleCallsOf(name: string, ms: number): Promise<PendingWork[]> {
  const deadline = Date.now() + ms;
  for (;;) {
    const open = [...rawCalls, ...fetches].filter(([work]) => work.owner === name);
    const left = deadline - Date.now();
    if (open.length === 0 || left <= 0) return open.map(([work]) => work);
    let handle: unknown;
    await Promise.race([
      Promise.all(open.map(([, settled]) => settled)),
      new Promise<void>((resolve) => { handle = runnerSetTimeout(resolve, left); }),
    ]);
    runnerClearTimeout(handle);
    // Continuations of what settled run before the next look.
    await new Promise<void>((resolve) => { runnerSetTimeout(resolve, 0); });
  }
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
