/**
 * Types for an lxapp's `mocks/index.ts`: the complete set of mock
 * handlers, one per call the app makes, answered in development when the
 * mock selection says so (`mocks/config.json`, `lingxia dev --mock`,
 * `lxdev mock`). Import them type-only; mocks never ship:
 *
 * ```ts
 * import type { Mocks } from '@lingxia/types/mocks';
 *
 * let signedIn = true; // module state: fresh on save, reset, restart and each test
 *
 * export default {
 *   'GET **\/devices': () => (signedIn ? { json: DEVICES } : { status: 401 }),
 *   'DELETE **\/sessions/current': () => { signedIn = false; return { status: 204 }; },
 *   'GET **\/legal': { json: [] },
 * } satisfies Mocks;
 * ```
 */
import type { NetworkRouteAnswer } from './automation/index.js';

/**
 * What a handler answers: exactly a route answer — `{ status?, headers?,
 * json | body, contentType?, delay? }`, `{ abort: 'failed' }`,
 * `{ hang: true }`, `{ sse: [...] }`, or `{ continue: true, patchJson? }`
 * (the real backend answers). One answer per call: `sequence`, `times`,
 * `match` and `bodyBase64` are scenario fields.
 */
export type MockAnswer = NetworkRouteAnswer;

/** The call a handler answers. */
export type MockRequest = {
  /** Fetch cancellation when supplied; null otherwise (including SSE). */
  signal: AbortSignal | null;
  /** Upper-case. */
  method: string;
  url: URL;
  /** What the app sent. */
  headers: Headers;
  /** The body as text; rejects for a stream, `Blob` or `FormData` body. */
  text(): Promise<string>;
  json<T = unknown>(): Promise<T>;
};

/** What a handler can use besides the request. */
export type MockContext = {
  /** The original `fetch`, not intercepted: proxy without recursing. */
  fetch: typeof fetch;
};

/**
 * Answers one call. Throwing, returning `undefined` or an invalid answer
 * fails the request (`TypeError: fetch failed`); it never falls through to
 * the real backend.
 */
export type MockHandler = (req: MockRequest, ctx: MockContext) => MockAnswer | Promise<MockAnswer>;

/** One native route's types: what the generated client records per route. */
export type NativeRouteTypes = {
  input: unknown;
  result: unknown;
  /** A stream route's progress events; `never` for a call route. */
  event: unknown;
  /** The route's error union, each code's fields under `data`. */
  error: { code: string; data?: unknown };
};

/**
 * Each native route's types, keyed by route name. The generated native client
 * (`.lingxia/native.logic.ts`) fills it, so a `'native notes.list'` handler is
 * checked against the route.
 */
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
export interface NativeMockRegistry {}

type NativeRoutes = NativeMockRegistry extends { routes: infer R } ? R : {};

type NativeErrorData<E, C> = Extract<E, { code: C }> extends { data?: infer D } ? D : undefined;

/** What a native handler can use besides the input. */
export type NativeMockContext<R extends NativeRouteTypes = NativeRouteTypes> = {
  route: string;
  /** The real route, when a host registered it (call routes). */
  call(input?: R['input']): Promise<R['result']>;
  /** The route's own error, to throw: it reaches the caller as the route's would. */
  error<C extends R['error']['code']>(code: C, data?: NativeErrorData<R['error'], C>, message?: string): Error;
};

/**
 * Answers one native call: the result, or a handler returning it. A stream
 * route's handler is a generator: it yields events and returns the result.
 * Throwing `ctx.error(code, data)` fails the call with the route's error.
 */
export type NativeMock<R extends NativeRouteTypes = NativeRouteTypes> =
  | R['result']
  | ((
      input: R['input'],
      ctx: NativeMockContext<R>,
    ) =>
      | R['result']
      | Promise<R['result']>
      | AsyncGenerator<R['event'], R['result'], void>
      | Generator<R['event'], R['result'], void>);

type JsonValue = string | number | boolean | null | readonly unknown[] | { readonly [key: string]: unknown };

/** The context of a glob key (`'native notes.*'`), which may match routes of any type. */
type AnyNativeMockContext = {
  route: string;
  call(input?: any): Promise<any>;
  error(code: string, data?: any, message?: string): Error;
};

/** A handler under a glob key: its routes' types are not known. */
type AnyNativeMock = JsonValue | ((input: any, ctx: AnyNativeMockContext) => unknown);

type HttpMethod = 'GET' | 'HEAD' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'OPTIONS' | 'CONNECT' | 'TRACE';

/** An HTTP handler key: `'GET **\/devices/*'`, `'* **\/x'`, `'POST /regex/'`. */
export type HttpMockKey = `${HttpMethod | Lowercase<HttpMethod> | '*'} ${string}`;

/**
 * The default export of `mocks/index.ts`: keys are HTTP targets
 * (`'GET **\/devices/*'`, `'* **\/x'`, `'POST /regex/'`) or native route
 * targets (`'native notes.list'`, `'native notes.*'`), tried in order; values
 * are answers or handlers.
 */
export type Mocks = { [key: HttpMockKey]: MockAnswer | MockHandler } & {
  [key: `native ${string}`]: AnyNativeMock;
} & { [K in keyof NativeRoutes as `native ${K & string}`]?: NativeMock<NativeRoutes[K] & NativeRouteTypes> };
