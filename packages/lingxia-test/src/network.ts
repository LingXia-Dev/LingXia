import type {
  NetworkDriver,
  NetworkRoute,
  NetworkRouteHandler,
  NetworkRoutePattern,
  NetworkRouteRequest,
  ScenarioCall,
} from "@lingxia/types/automation";
import { ActionDeadline, TimeoutError } from "./deadline.js";
import { truncate } from "./format.js";
import { callerLocation, displayLocation } from "./ids.js";
import type { NetworkCall, TestNetwork, TestRoute, WaitForCallOptions } from "./types.js";
import { DEFAULT_ACTION_TIMEOUT_MS, DEFAULT_POLL_INTERVAL_MS } from "./version.js";
import { runnerClearTimeout, runnerSetTimeout } from "./pending.js";

/** The fixture surface the network, scenario and clock wrappers need. */
export interface NetworkHost {
  act<T>(name: string, detail: string, op: () => T | Promise<T>): Promise<T>;
  /** Milliseconds left in the spec (or cleanup) budget. */
  budgetRoom(): number;
  /** Stop recording actions while a wait polls; `resumeActions` undoes it. */
  silenceActions(): void;
  resumeActions(): void;
  /** A note for the run's diagnostics, beside the trace. */
  diagnostic(phase: string, message: string): void | Promise<void>;
}

/**
 * Routes one spec installed, across every `t.app` / `t.automation.lxapp()`
 * wrapper it created. The runner removes them when the spec ends — whether
 * or not its body settled, apart from the spec's own cleanup — so a route
 * never bleeds into the next spec of the same run.
 */
export class NetworkScope {
  readonly routes = new Map<number, NetworkRoute>();

  track(route: NetworkRoute): void {
    this.routes.set(route.id, route);
  }

  /**
   * Remove every tracked route; a route already gone (expired through
   * `times`, removed by the spec) counts as removed. Rejects naming the
   * routes whose removal failed.
   */
  async reclaim(): Promise<void> {
    const failed: string[] = [];
    for (const [id, route] of [...this.routes]) {
      // Raw handle: the fixture is closed by now.
      try {
        await route.unroute();
        this.routes.delete(id);
      } catch (error) {
        failed.push(`${route.pattern}: ${errorText(error)}`);
      }
    }
    if (failed.length > 0) throw new Error(`network routes were not removed (${failed.join("; ")})`);
  }
}

export function errorText(error: unknown): string {
  return String((error as Error)?.message ?? error);
}

/** A request body as the spec reads it: parsed when it is JSON. */
function parsedBody(body: string | null): unknown {
  if (body === null) return null;
  try {
    return JSON.parse(body);
  } catch {
    return body;
  }
}

/** One request a route handled, as a `NetworkCall`. */
export function routeCall(request: NetworkRouteRequest): NetworkCall {
  return {
    seq: request.seq,
    time: request.timestamp,
    kind: "http",
    method: request.method,
    url: request.url,
    status: request.status,
    body: parsedBody(request.body),
    headers: request.headers,
    // A plain `continue` let the real backend answer.
    answeredBy: request.action === "continue" ? "real" : "route",
  };
}

/** One call that reached a scenario, as a `NetworkCall`. */
export function scenarioCall(call: ScenarioCall): NetworkCall {
  const answeredBy: NetworkCall["answeredBy"] = call.rule !== null
    ? "rule"
    : call.answeredBy.startsWith("route")
      ? "route"
      : call.answeredBy.startsWith("mock") || call.answeredBy === "function mock"
        ? "mock"
        : call.answeredBy === "function real"
          ? "real"
          : call.kind === "function"
            ? "companion"
            : "real";
  const out: NetworkCall = { seq: call.seq, time: call.time, kind: call.kind, answeredBy };
  if (call.kind === "function") {
    out.function = call.function;
    if (call.args !== undefined) out.body = call.args;
    if (call.outcome !== undefined) out.outcome = call.outcome;
  } else {
    out.method = call.method;
    out.url = call.url;
    out.status = call.status ?? null;
    if (call.body !== undefined) out.body = call.body;
  }
  if (call.rule !== null) out.rule = call.rule;
  if (call.noMatch) out.noMatch = call.noMatch;
  return out;
}

/** `GET https://h/x → 200 (rule 2)`, for failure messages. */
export function describeCall(call: NetworkCall): string {
  const by = call.answeredBy === "rule" && call.rule !== undefined ? `rule ${call.rule}` : call.answeredBy;
  const head = call.kind === "function"
    ? `function ${call.function ?? "?"} → ${call.outcome ?? "no answer"}`
    : `${call.method ?? "?"} ${call.url ?? "?"} → ${call.status ?? "no answer"}`;
  return `${head} (${by})${call.noMatch ? `; ${call.noMatch}` : ""}`;
}

/** How many recent calls a `waitForCall` timeout lists. */
const LISTED_CALLS = 10;
/**
 * The most a `waitForCall` timeout spends reading recent calls for its
 * message, past its own timeout and never past the spec's budget.
 */
const EVIDENCE_MS = 100;

/** Calls after a cursor, and how far the log they come from was trimmed. */
export interface CallWindowRead {
  calls: NetworkCall[];
  /** The highest `seq` the log dropped (0 when none). */
  droppedThrough: number;
}

/** Where one handle's (or scenario target's) `waitForCall` stands. */
export interface CallCursor {
  /** `seq` of the last call handed out; 0 before the first. */
  after: number;
  /** Calls handed out so far. */
  taken: number;
}

/**
 * Poll `read(cursor.after)` until it lists a call and hand out the oldest.
 * Each handle (and scenario target) keeps its own cursor, so successive
 * waits return successive calls, including ones made before the wait
 * started. The host logs are bounded; when they dropped calls past the
 * cursor, the next call cannot be told and the wait says so instead of
 * handing out a later one.
 *
 * Every read and pause shares one deadline: a read that never returns
 * fails the wait on time, and one that returns after the deadline does not
 * count. The recent calls a timeout lists get a small slice of their own.
 */
export function waitForNextCall(
  host: NetworkHost,
  verb: string,
  what: string,
  read: (after: number) => Promise<CallWindowRead>,
  recent: () => Promise<NetworkCall[]>,
  cursor: CallCursor,
  options: WaitForCallOptions = {},
): Promise<NetworkCall> {
  const location = callerLocation();
  const at = `at ${displayLocation(location.file, location.line, location.column)}`;
  const requested = options.timeout ?? DEFAULT_ACTION_TIMEOUT_MS;
  const interval = options.interval ?? DEFAULT_POLL_INTERVAL_MS;
  if (!Number.isFinite(requested) || requested <= 0 || !Number.isFinite(interval) || interval <= 0) {
    throw new TypeError("waitForCall timeout and interval must be positive finite numbers");
  }
  return host.act(verb, what, async () => {
    const deadline = new ActionDeadline(requested, host.budgetRoom());
    let stalled = false;
    host.silenceActions();
    try {
      for (;;) {
        if (deadline.expired()) break;
        let window: CallWindowRead;
        try {
          window = await deadline.call(`${verb} reading calls`, () => read(cursor.after), () => at);
        } catch (error) {
          if (!(error instanceof TimeoutError) || !deadline.expired()) throw error;
          stalled = true;
          break;
        }
        // A read that came back after the deadline does not count.
        if (deadline.expired()) break;
        if (window.droppedThrough > cursor.after) {
          const lost = window.droppedThrough - cursor.after;
          const skippedFrom = cursor.after;
          const range = lost === 1 ? `seq ${window.droppedThrough}` : `seq ${skippedFrom + 1}–${window.droppedThrough}`;
          cursor.after = window.droppedThrough;
          throw new Error([
            `${what}: calls after ${skippedFrom === 0 ? "the start" : `the last one waitForCall returned (seq ${skippedFrom})`} ` +
              `were dropped from a bounded call log before they were read (${range}, ` +
              `${lost} ${lost === 1 ? "entry" : "entries"} of that log), so the next call cannot be told.`,
            "Wait for calls as they happen, or make fewer calls between waits; the next waitForCall resumes after the gap.",
            at,
          ].join("\n"));
        }
        const next = window.calls.find((call) => call.seq > cursor.after);
        if (next) {
          cursor.after = next.seq;
          cursor.taken += 1;
          return next;
        }
        const pause = Math.min(interval, deadline.remaining());
        if (pause <= 0) break;
        await new Promise<void>((resolve) => { runnerSetTimeout(resolve, pause); });
      }
    } finally {
      host.resumeActions();
    }
    const elapsed = deadline.elapsed();
    // A read that stalled says the driver is not answering; asking it for
    // recent calls would only spend more time.
    const evidence = stalled
      ? "The last read of its calls had not returned when the time ran out, so recent calls were not read."
      : await recentCalls(recent, Math.min(EVIDENCE_MS, Math.floor(host.budgetRoom())));
    const seen = cursor.taken;
    throw new TimeoutError([
      `${what}: no new call within ${elapsed}ms` +
        (seen > 0 ? ` (${seen} earlier ${seen === 1 ? "call was" : "calls were"} already returned by waitForCall).` : "."),
      evidence,
      deadline.clampNote(),
      at,
    ].filter(Boolean).join("\n"));
  });
}

/** The lines listing recent calls for a timeout, read within `ms`. */
async function recentCalls(recent: () => Promise<NetworkCall[]>, ms: number): Promise<string> {
  if (ms < 1) return "Recent calls were not read: the spec's budget is spent.";
  let handle: unknown;
  const late = new Promise<"late">((resolve) => { handle = runnerSetTimeout(() => resolve("late"), ms); });
  let listed: NetworkCall[] | "late";
  try {
    const task = Promise.resolve().then(recent);
    task.catch(() => {});
    listed = await Promise.race([task, late]);
  } catch {
    // The listing explains the timeout; it never replaces it.
    return "Recent calls could not be read.";
  } finally {
    runnerClearTimeout(handle);
  }
  if (listed === "late") return `Recent calls were not read: the read took longer than ${ms}ms.`;
  const tail = listed.slice(-LISTED_CALLS);
  return tail.length > 0
    ? `Recent calls:\n${tail.map((call) => `  ${describeCall(call)}`).join("\n")}`
    : "No call reached it.";
}

/**
 * `resolve` reads the raw driver lazily, inside each traced call, so an app
 * that closed since `t.app` was read fails that call, not the read.
 */
export function wrapNetwork(resolve: () => NetworkDriver, host: NetworkHost, scope: NetworkScope): TestNetwork {
  const driver = resolve;
  const wrapRoute = (route: NetworkRoute): TestRoute => {
    const cursor: CallCursor = { after: 0, taken: 0 };
    const calls = async () => (await route.requests()).map(routeCall);
    const after = async (seq: number): Promise<CallWindowRead> => {
      const window = await route.requestsAfter(seq);
      return { calls: window.requests.map(routeCall), droppedThrough: window.droppedThrough };
    };
    return {
      get id() { return route.id; },
      get pattern() { return route.pattern; },
      remove: () => host.act("network.remove", route.pattern, async () => { await route.unroute(); }),
      calls: () => host.act("network.calls", route.pattern, calls),
      waitForCall: (options?: WaitForCallOptions) =>
        waitForNextCall(host, "network.waitForCall", `route ${route.pattern}`, after, calls, cursor, options),
    };
  };
  return {
    route: (pattern: NetworkRoutePattern, handler: NetworkRouteHandler) =>
      host.act("network.route", describeRoute(pattern, handler), async () => {
        const route = await driver().route(pattern, handler);
        scope.track(route);
        return wrapRoute(route);
      }),
    removeAll: () => host.act("network.removeAll", "", async () => { await driver().unrouteAll(); }),
    // Spec-scoped: the host log spans the whole run.
    calls: () =>
      host.act("network.calls", "", async () =>
        (await driver().requests())
          .filter((entry: NetworkRouteRequest) => scope.routes.has(entry.routeId))
          .map(routeCall)),
  };
}

function describePattern(pattern: NetworkRoutePattern): string {
  if (typeof pattern === "string") return pattern;
  if (pattern instanceof RegExp) return String(pattern);
  const url = pattern.url instanceof RegExp ? String(pattern.url) : pattern.url;
  return pattern.method ? `${pattern.method.toUpperCase()} ${url}` : url;
}

function describeHandler(handler: NetworkRouteHandler): string {
  if (handler.sequence !== undefined) return `sequence of ${handler.sequence.length}`;
  if (handler.sse !== undefined) return `sse (${handler.sse.length} items)`;
  if (handler.abort !== undefined) return `abort ${String(handler.abort)}`;
  if (handler.hang !== undefined) return "hang";
  if (handler.continue !== undefined) return handler.patchJson !== undefined ? "continue + patchJson" : "continue";
  const status = String(handler.status ?? 200);
  return handler.delay ? `${status} after ${handler.delay}ms` : status;
}

function describeRoute(pattern: NetworkRoutePattern, handler: NetworkRouteHandler): string {
  return truncate(`${describePattern(pattern)} → ${describeHandler(handler)}`, 80);
}

