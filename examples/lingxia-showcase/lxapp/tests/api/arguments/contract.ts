import { expect, test, type JsonValue } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';
import { bindFixture } from '@lingxia/test/showcase/helpers/poll.js';

/** Stand-ins for values JSON cannot carry; Logic rebuilds them before the call. */
export const FN = { $lx: 'fn' } as const;
/** An `AbortSignal` that has already aborted. */
export const ABORTED = { $lx: 'aborted' } as const;

/** `{ns}` in any string argument becomes this spec's own storage namespace. */
export const NS = '{ns}';

/** Codes a call answers before it checks the host can do it. */
const ARGUMENT_STAGE = new Set(['E_INVALID_ARG', 'E_ABORT']);

export interface RowOptions {
  /** Read (or call, when a method) this member off the result and settle it too. */
  readonly then?: string;
  /** Run this `lx` member instead of the table's, to set up state for later rows. */
  readonly via?: string;
  /**
   * A known product bug: the row must still miss `code`. When it starts to
   * answer `code`, the spec fails so the marker gets removed.
   */
  readonly bug?: string;
}

/** `[label, args, expected]`: `expected` is a stable `E_*` code, or `'ok'` for a call that must succeed. */
export type Row = readonly [label: string, args: readonly JsonValue[], expected: string, options?: RowOptions];

export interface ApiContract {
  /**
   * A host without this `lx.supports` feature still refuses bad arguments
   * (`E_INVALID_ARG`, or `E_ABORT` for a pre-aborted signal), so a broken call
   * fails the same everywhere; every other row answers `E_NOT_SUPPORTED` there.
   */
  readonly feature?: string;
  /** The feature gate runs before argument checks: an unsupported host answers every row `E_NOT_SUPPORTED`. */
  readonly gatedFirst?: boolean;
  /** Destructive or external: only refusals are tested, never a call that would act. */
  readonly refuseOnly?: boolean;
  readonly rows: readonly Row[];
  /** `[api, args]` run after the rows whatever they answered. */
  readonly cleanup?: readonly (readonly [string, readonly JsonValue[]])[];
}

interface Call {
  readonly label: string;
  readonly api: string;
  readonly args: readonly JsonValue[];
  readonly then: string | null;
}

/**
 * One spec per `lx` function in `table`, keyed by its path below `lx`. Each
 * spec runs its rows in order and compares the whole table at once, so one
 * moved code names every other row that moved with it.
 */
export function argumentContracts(table: Readonly<Record<string, ApiContract | readonly Row[]>>): void {
  for (const [api, entry] of Object.entries(table)) {
    const contract: ApiContract = Array.isArray(entry) ? { rows: entry as readonly Row[] } : entry as ApiContract;
    const id = `ARGS-${api.replace(/\./g, '-').toUpperCase()}`;
    test(`lx.${api} answers its arguments with stable codes`, {
      id,
      covers: [`args:lx.${api}`],
      app: SHOWCASE_APP_ID,
    }, async (t) => {
      if (contract.refuseOnly && contract.rows.some(([, , expected, options]) => expected === 'ok' && !options?.via)) {
        throw new Error(`lx.${api} is refuse-only: an 'ok' row would act`);
      }
      const { app, namespace } = bindFixture(t, id);
      const calls: Call[] = contract.rows.map(([label, args, , options]) => ({
        label, api: options?.via ?? api, args, then: options?.then ?? null,
      }));
      const cleanup: Call[] = (contract.cleanup ?? []).map(([cleanupApi, args]) => ({
        label: 'cleanup', api: cleanupApi, args, then: null,
      }));
      // Strict dialogs: a regression that accepts a bad dialog call fails here instead of drawing.
      const run = await app.dialogs.withAnswerMode({ modals: 'strict', actionSheets: 'strict' }, () =>
        app.logic.eval(async ({ lx }, calls, cleanup, namespace, feature) => {
          const live = (value: unknown): unknown => {
            if (typeof value === 'string') return value.split('{ns}').join(namespace);
            if (Array.isArray(value)) return value.map(live);
            if (value && typeof value === 'object') {
              const marker = (value as { $lx?: string }).$lx;
              if (marker === 'fn') return () => {};
              if (marker === 'aborted') {
                const controller = new AbortController();
                controller.abort();
                return controller.signal;
              }
              const out: Record<string, unknown> = {};
              for (const [key, inner] of Object.entries(value)) out[key] = live(inner);
              return out;
            }
            return value;
          };
          // A call that neither settles nor throws has started acting (a picker, a dialog).
          const settle = async (value: unknown): Promise<unknown> => {
            if (!value || typeof (value as { then?: unknown }).then !== 'function') return value;
            let timer: ReturnType<typeof setTimeout> | undefined;
            const pending = new Promise((_, reject) => {
              timer = setTimeout(() => reject({ code: 'PENDING' }), 3_000);
            });
            try {
              return await Promise.race([value, pending]);
            } finally {
              if (timer !== undefined) clearTimeout(timer);
            }
          };
          const release = (value: unknown) => {
            // An unsubscribe; a callable host object (an `LxFile`) refuses the call, which is fine.
            if (typeof value === 'function') {
              try { (value as () => void)(); } catch { /* not an unsubscribe */ }
            }
            else if (value && typeof value === 'object') {
              for (const method of ['close', 'cancel']) {
                const fn = (value as Record<string, unknown>)[method];
                if (typeof fn === 'function') {
                  try { void (fn as () => unknown).call(value); } catch { /* already settled */ }
                  return;
                }
              }
            }
          };
          const supported = feature === null || lx.supports(feature as never);
          const call = async (item: unknown) => {
            const { api, args, then } = item as { api: string; args: unknown[]; then: string | null };
            try {
              const path = api.split('.');
              let owner: unknown = lx;
              for (const key of path.slice(0, -1)) owner = (owner as Record<string, unknown> | undefined)?.[key];
              const fn = (owner as Record<string, (...values: unknown[]) => unknown> | undefined)?.[path[path.length - 1]];
              // Optional namespaces are only injected where the host has the feature.
              if (typeof fn !== 'function') return 'ABSENT';
              let value = await settle(fn.apply(owner, live(args) as unknown[]));
              if (then !== null) {
                const member = (value as Record<string, unknown>)[then];
                value = await settle(typeof member === 'function' ? (member as () => unknown).call(value) : member);
              }
              release(value);
              return 'ok';
            } catch (error) {
              const { code, message } = error as { code?: string; message?: string };
              return code ? String(code) : `THROWN ${String(message ?? error)}`;
            }
          };
          const answers: string[] = [];
          for (const item of calls) answers.push(await call(item));
          for (const item of cleanup) await call(item);
          return { supported, answers };
        }, calls as unknown as JsonValue[], cleanup as unknown as JsonValue[], namespace, contract.feature ?? null),
      );

      const observed: string[] = [];
      const wanted: string[] = [];
      contract.rows.forEach(([label, , expected, options], index) => {
        const gated = !run.supported && !options?.via && (contract.gatedFirst || !ARGUMENT_STAGE.has(expected));
        const code = gated ? 'E_NOT_SUPPORTED' : expected;
        // A host without the feature may leave the namespace out altogether.
        const absent = run.answers[index] === 'ABSENT';
        const answer = absent && !run.supported ? code : absent ? `lx.${api} is missing` : run.answers[index];
        observed.push(`${label} = ${answer}`);
        if (options?.bug && run.supported) {
          // Still broken is the expectation; a fixed bug must lose its marker.
          wanted.push(answer === code ? `${label} = FIXED, remove bug marker (${options.bug})` : `${label} = ${answer}`);
        } else {
          wanted.push(`${label} = ${code}`);
        }
      });
      expect(observed).toEqual(wanted);
    });
  }
}

/**
 * The rows every `watch`/`on*` subscription shares: a callback, then `{ signal }`.
 * A pre-aborted signal subscribes nothing and still returns an unsubscribe.
 */
export function subscriptionRows(): readonly Row[] {
  return [
    ['no callback', [], 'E_INVALID_ARG'],
    ['number callback', [1], 'E_INVALID_ARG'],
    ['options not object', [FN, 'x'], 'E_INVALID_ARG'],
    ['signal not a signal', [FN, { signal: 1 }], 'E_INVALID_ARG'],
    ['callback', [FN], 'ok'],
    ['aborted signal', [FN, { signal: ABORTED }], 'ok'],
  ];
}
