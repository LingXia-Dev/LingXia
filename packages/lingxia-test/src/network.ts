import type {
  NetworkDriver,
  NetworkRoute,
  NetworkRouteHandler,
  NetworkRoutePattern,
  NetworkRouteRequest,
} from "@lingxia/types/automation";
import { truncate } from "./format.js";

/** The fixture surface the network wrapper needs. */
export interface NetworkHost {
  act<T>(name: string, detail: string, op: () => T | Promise<T>): Promise<T>;
  defer(cleanup: () => void | Promise<void>): void;
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

/**
 * `resolve` reads the raw driver lazily, inside each traced call: a host
 * without test routing then fails that call, never the `t.app.network` read.
 */
export function wrapNetwork(resolve: () => NetworkDriver | undefined, host: NetworkHost, scope: NetworkScope): NetworkDriver {
  const driver = (): NetworkDriver => {
    const network = resolve();
    if (!network) throw new Error("t.app.network is not supported by this host");
    return network;
  };
  const wrapRoute = (route: NetworkRoute): NetworkRoute => ({
    get id() { return route.id; },
    get pattern() { return route.pattern; },
    unroute: () => host.act("network.unroute", route.pattern, () => route.unroute()),
    requests: () => host.act("network.requests", route.pattern, () => route.requests()),
  });
  return {
    route: (pattern: NetworkRoutePattern, handler: NetworkRouteHandler) =>
      host.act("network.route", describeRoute(pattern, handler), async () => {
        const route = await driver().route(pattern, handler);
        scope.track(host, route);
        return wrapRoute(route);
      }),
    unrouteAll: () => host.act("network.unrouteAll", "", () => driver().unrouteAll()),
    // Spec-scoped: the host log spans the whole run.
    requests: () =>
      host.act("network.requests", "", async () =>
        (await driver().requests()).filter((entry: NetworkRouteRequest) => scope.routes.has(entry.routeId))),
    // Run-scoped, like the raw driver: `lxdev test --openapi` turns capture on.
    captureResponses: (options) => host.act("network.captureResponses", "", () => driver().captureResponses(options)),
    responses: (options) => host.act("network.responses", "", () => driver().responses(options)),
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
