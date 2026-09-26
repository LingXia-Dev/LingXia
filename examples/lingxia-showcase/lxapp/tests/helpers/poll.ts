import { expect, type Fixture, type TestApp } from '@lingxia/test';

export interface EventuallyOptions<T> {
  timeoutMs?: number;
  intervalMs?: number;
  describe: string;
  retryIf?: (error: unknown) => boolean;
  render?: (value: T) => string;
}

function errorText(error: unknown): string {
  return error instanceof Error ? `${error.name}: ${error.message}` : String(error);
}

export async function eventually<T>(
  read: () => T | Promise<T>,
  accept: (value: T) => boolean,
  options: EventuallyOptions<T>,
): Promise<T> {
  const timeoutMs = options.timeoutMs ?? 10_000;
  const intervalMs = options.intervalMs ?? 50;
  const deadline = Date.now() + timeoutMs;
  let lastValue: T | undefined;
  let lastError: unknown;

  while (Date.now() < deadline) {
    try {
      lastValue = await read();
      lastError = undefined;
      if (accept(lastValue)) return lastValue;
    } catch (error) {
      if (!options.retryIf?.(error)) throw error;
      lastError = error;
    }
    await new Promise<void>((resolve) => setTimeout(resolve, intervalMs));
  }

  const observed = lastError === undefined
    ? options.render?.(lastValue as T) ?? JSON.stringify(lastValue)
    : errorText(lastError);
  throw new Error(`Timed out waiting for ${options.describe}; last observed: ${observed}`);
}

/**
 * What a Logic probe that catches its own rejection resolves to. A rejection
 * thrown inside `t.app.logic.eval(fn)` reaches the spec as the eval's own
 * failure, so a probe that checks an API's error code catches it in `fn`:
 *
 * ```ts
 * const outcome: Caught = await t.app.logic.eval(async ({ lx }) => {
 *   try {
 *     return { ok: true, value: await lx.host.setBadge(2) };
 *   } catch (error) {
 *     const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
 *     return { ok: false, code, message: String(message ?? error), data };
 *   }
 * });
 * ```
 */
export type Caught<T = unknown> =
  | { ok: true; value?: T; code?: undefined; message?: undefined; data?: undefined }
  | { ok: false; value?: undefined; code?: string; message: string; data?: unknown };

/** Schedule `lx.reLaunch` without awaiting the torn-down eval context. */
export async function relaunchFromLogic(
  app: TestApp,
  page: string,
  query?: Record<string, string>,
): Promise<void> {
  await app.logic.eval(({ lx }, page, query) => {
    void lx.reLaunch(query === null ? { page } : { page, query });
    return 'scheduled';
  }, page, query ?? null);
}

export async function expectReject(
  operation: () => Promise<unknown>,
  expected: { code?: string; message?: string | RegExp },
): Promise<void> {
  let received: unknown;
  try {
    await operation();
  } catch (error) {
    received = error;
  }
  expect(received).toBeDefined();
  const record = received as { code?: unknown; message?: unknown };
  if (expected.code !== undefined) expect(record.code).toBe(expected.code);
  if (typeof expected.message === 'string') expect(String(record.message)).toContain(expected.message);
  if (expected.message instanceof RegExp) expect(String(record.message)).toMatch(expected.message);
}

let sequence = 0;

export function specNamespace(id: string): string {
  sequence += 1;
  return `${id.replace(/[^a-zA-Z0-9]+/g, '-')}-${Date.now()}-${sequence}`;
}

/** Attach through the fixture so the artifact is embedded in the HTML report;
 *  a raw host attach only lands on disk and never reaches a reader. */
export async function attachShot(
  t: Fixture,
  name: string,
  artifact: { mimeType: string; base64: string },
): Promise<void> {
  await t.attach(name, artifact);
}

export function bindFixture(t: Fixture, id: string): {
  app: TestApp;
  namespace: string;
  defer: (cleanup: () => void | Promise<void>) => void;
} {
  return {
    app: t.app,
    namespace: specNamespace(id),
    defer: (cleanup) => t.defer(cleanup),
  };
}
