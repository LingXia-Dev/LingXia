import type {
  NetworkDriver,
  NetworkRoute,
  NetworkRouteHandler,
  NetworkRoutePattern,
  NetworkRouteRequest,
  ScenarioCall,
} from "@lingxia/types/automation";
import { TimeoutError } from "./deadline.js";
import { truncate } from "./format.js";
import { callerLocation, displayLocation } from "./ids.js";
import type { NetworkCall, TestNetwork, TestRoute, WaitForCallOptions } from "./types.js";
import { DEFAULT_ACTION_TIMEOUT_MS, DEFAULT_POLL_INTERVAL_MS } from "./version.js";

/** The fixture surface the network, scenario and clock wrappers need. */
export interface NetworkHost {
  act<T>(name: string, detail: string, op: () => T | Promise<T>): Promise<T>;
  defer(cleanup: () => void | Promise<void>): void;
  /** Milliseconds left in the spec (or cleanup) budget. */
  budgetRoom(): number;
  /** Stop recording actions while a wait polls; `resumeActions` undoes it. */
  silenceActions(): void;
  resumeActions(): void;
  /** A note for the run's diagnostics, beside the trace. */
  diagnostic(phase: string, message: string): void | Promise<void>;
}

/**
 * Routes one spec installed, across every `t.app` / `t.apps` wrapper it
 * created. They are removed when the spec ends, so a route never bleeds into
 * the next spec of the same run; the host drops whatever is left at run end.
 */
export class NetworkScope {
  readonly routes = new Map<number, NetworkRoute>();
  private cleanupRegistered = false;

  track(host: NetworkHost, route: NetworkRoute): void {
    this.routes.set(route.id, route);
    if (this.cleanupRegistered) return;
    this.cleanupRegistered = true;
    host.defer(async () => {
      for (const route of this.routes.values()) {
        // Raw handle: cleanup runs after the fixture closed its action guard,
        // and a route that already expired resolves false, not an error.
        try { await route.unroute(); } catch { /* the run end clears it */ }
      }
    });
  }
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
      : call.kind === "function"
        ? "companion"
        : "real";
  const out: NetworkCall = { time: call.time, kind: call.kind, answeredBy };
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
 * Poll `read` until it lists more than `cursor.taken` calls and hand out the
 * next one. Each handle (and scenario target) keeps its own cursor, so
 * successive waits return successive calls, including ones made before the
 * wait started.
 */
export function waitForNextCall(
  host: NetworkHost,
  verb: string,
  what: string,
  read: () => Promise<NetworkCall[]>,
  recent: () => Promise<NetworkCall[]>,
  cursor: { taken: number },
  options: WaitForCallOptions = {},
): Promise<NetworkCall> {
  const location = callerLocation();
  const requested = options.timeout ?? DEFAULT_ACTION_TIMEOUT_MS;
  const interval = options.interval ?? DEFAULT_POLL_INTERVAL_MS;
  if (!Number.isFinite(requested) || requested <= 0 || !Number.isFinite(interval) || interval <= 0) {
    throw new TypeError("waitForCall timeout and interval must be positive finite numbers");
  }
  const timeout = Math.max(1, Math.min(requested, host.budgetRoom()));
  return host.act(verb, what, async () => {
    const started = Date.now();
    let seen = 0;
    host.silenceActions();
    try {
      for (;;) {
        const calls = await read();
        seen = calls.length;
        const next = calls[cursor.taken];
        if (next) {
          cursor.taken += 1;
          return next;
        }
        if (Date.now() - started + interval > timeout) break;
        await new Promise((resolve) => setTimeout(resolve, interval));
      }
    } finally {
      host.resumeActions();
    }
    let listed: NetworkCall[] = [];
    try {
      listed = (await recent()).slice(-LISTED_CALLS);
    } catch {
      // The listing explains the timeout; it never replaces it.
    }
    throw new TimeoutError([
      `${what}: no new call within ${Date.now() - started}ms` +
        (seen > 0 ? ` (${seen} earlier ${seen === 1 ? "call was" : "calls were"} already returned by waitForCall).` : "."),
      listed.length > 0
        ? `Recent calls:\n${listed.map((call) => `  ${describeCall(call)}`).join("\n")}`
        : "No call reached it.",
      timeout < requested ? `Clamped from ${requested}ms to the spec's remaining budget.` : undefined,
      `at ${displayLocation(location.file, location.line, location.column)}`,
    ].filter(Boolean).join("\n"));
  });
}

/**
 * `resolve` reads the raw driver lazily, inside each traced call: a host
 * without test routing then fails that call, never the `t.app.network` read.
 */
export function wrapNetwork(resolve: () => NetworkDriver | undefined, host: NetworkHost, scope: NetworkScope): TestNetwork {
  const driver = (): NetworkDriver => {
    const network = resolve();
    if (!network) throw new Error("t.app.network is not supported by this host");
    return network;
  };
  const wrapRoute = (route: NetworkRoute): TestRoute => {
    const cursor = { taken: 0 };
    const calls = async () => (await route.requests()).map(routeCall);
    return {
      get id() { return route.id; },
      get pattern() { return route.pattern; },
      unroute: () => host.act("network.unroute", route.pattern, async () => { await route.unroute(); }),
      calls: () => host.act("network.calls", route.pattern, calls),
      waitForCall: (options?: WaitForCallOptions) =>
        waitForNextCall(host, "network.waitForCall", `route ${route.pattern}`, calls, calls, cursor, options),
    };
  };
  return {
    route: (pattern: NetworkRoutePattern, handler: NetworkRouteHandler) =>
      host.act("network.route", describeRoute(pattern, handler), async () => {
        const route = await driver().route(pattern, handler);
        scope.track(host, route);
        return wrapRoute(route);
      }),
    unrouteAll: () => host.act("network.unrouteAll", "", async () => { await driver().unrouteAll(); }),
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

