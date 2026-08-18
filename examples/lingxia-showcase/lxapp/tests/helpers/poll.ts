import { expect, type Fixture } from '@lingxia/test';
import type { LxAppDriver } from 'lingxia-types/automation';

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

export type CaughtEval = {
  ok: boolean;
  value?: unknown;
  code?: unknown;
  message?: string;
  data?: unknown;
};

/** Run Logic and return `{ ok, code, data }` instead of throwing across eval. */
/** Schedule `lx.reLaunch` without awaiting the torn-down eval context. */
export async function relaunchFromLogic(
  app: LxAppDriver,
  page: string,
  query?: Record<string, string>,
): Promise<void> {
  const queryLiteral = query === undefined ? '' : `, query: ${JSON.stringify(query)}`;
  await app.eval({
    script: `void lx.reLaunch({ page: ${JSON.stringify(page)}${queryLiteral} }); return 'scheduled';`,
  });
}

export async function evalCaught(app: LxAppDriver, body: string): Promise<CaughtEval> {
  return app.eval({
    script: `
      try {
        const value = await (async () => { ${body} })();
        return { ok: true, value };
      } catch (error) {
        return {
          ok: false,
          code: error && error.code,
          message: String(error && error.message || error),
          data: error && error.data,
        };
      }
    `,
  }) as Promise<CaughtEval>;
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

export async function hostAttach(
  name: string,
  artifact: { mimeType: string; base64: string },
): Promise<void> {
  const host = globalThis.__LINGXIA_AUTOMATION_HOST__;
  if (!host?.attach) return;
  await host.attach(`attachments/${name}`, artifact);
}

export function bindFixture(t: Fixture, id: string): {
  app: LxAppDriver;
  namespace: string;
  defer: (cleanup: () => void | Promise<void>) => void;
} {
  return {
    app: t.app,
    namespace: specNamespace(id),
    defer: (cleanup) => t.defer(cleanup),
  };
}
