/**
 * Types for an lxapp's `mocks/index.ts`: the complete set of mock
 * handlers, one per call the app makes, answered in development when the
 * mock selection says so (`mocks/config.json`, `lingxia dev --mock`,
 * `lxdev mock`). Import them type-only; mocks never ship:
 *
 * ```ts
 * import type { Mocks } from '@lingxia/types/mocks';
 *
 * let signedIn = true; // module state: fresh on save, reset, restart and each spec
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

/**
 * The default export of `mocks/index.ts`: keys are HTTP targets
 * (`'GET **\/devices/*'`, `'* **\/x'`, `'POST /regex/'`), tried in order;
 * values are answers or handlers.
 */
export type Mocks = Record<string, MockAnswer | MockHandler>;
