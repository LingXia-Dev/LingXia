import type { Lx } from '../index.js';
import { LX_MOCK_SHAPE } from './lx-mock-shape.js';

/** `T` with every namespace optional, down to its members; a function is given whole or not at all. */
export type LxMockOverrides<T = Lx> = {
  [K in keyof T]?: T[K] extends (...args: never[]) => unknown ? T[K] : T[K] extends object ? LxMockOverrides<T[K]> : T[K];
};

function notSupported(path: string): Error & { code: 'E_NOT_SUPPORTED' } {
  return Object.assign(new Error(`${path} is not mocked: give it in createLxMock({ ... })`), {
    code: 'E_NOT_SUPPORTED' as const,
  });
}

function stand(path: string, overrides: Record<string, unknown> | undefined): unknown {
  return new Proxy(Object.create(null) as Record<string, unknown>, {
    get(_, key) {
      if (typeof key !== 'string') return undefined;
      const member = `${path}.${key}`;
      const given = overrides?.[key];
      const kind = LX_MOCK_SHAPE[member];
      if (given !== undefined) {
        // A namespace given in part still stands in for the members it leaves out.
        if (kind === 'namespace' && typeof given === 'object' && given !== null) {
          return stand(member, given as Record<string, unknown>);
        }
        return given;
      }
      switch (kind) {
        case 'namespace':
          return stand(member, undefined);
        case 'async':
          return () => Promise.reject(notSupported(member));
        case 'sync':
          return () => {
            throw notSupported(member);
          };
        case 'value':
          throw notSupported(member);
        default:
          return undefined;
      }
    },
    has(_, key) {
      return typeof key === 'string' && (`${path}.${key}` in LX_MOCK_SHAPE || (overrides !== undefined && key in overrides));
    },
  });
}

/**
 * An `lx` for a Node test. Members you give answer as given; any other member
 * rejects (a synchronous one throws, a property read throws) `E_NOT_SUPPORTED`
 * naming itself. Install it with `setLx` from `@lingxia/logic/testing`, or pass
 * it to `mount`.
 * @example
 * ```ts
 * const lx = createLxMock({ pick: { file: async () => ({ status: 'ok', file }) } });
 * ```
 */
export function createLxMock(overrides: LxMockOverrides = {}): Lx {
  return stand('lx', overrides as Record<string, unknown>) as Lx;
}
