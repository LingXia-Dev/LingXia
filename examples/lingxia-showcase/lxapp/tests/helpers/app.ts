import type { LxAppDriver } from '@lingxia/types/automation';

export const SHOWCASE_APP_ID = 'lingxia-showcase';

/** What a raw eval may take: the fixture gives its evals a third of a 30 s spec. */
const EVAL_BUDGET_MS = 10_000;

/**
 * A member read off a raw driver, methods bound to it. Native driver objects
 * are callable (`typeof` is `function`), so a namespace a getter returns
 * (`nav`, `pointer`, …) must be returned as is, never bound as a method.
 */
function member(target: object, prop: PropertyKey): unknown {
  const value = Reflect.get(target, prop, target);
  let owner: object | null = target;
  while (owner) {
    const descriptor = Object.getOwnPropertyDescriptor(owner, prop);
    if (descriptor) return typeof descriptor.get === 'function' || typeof value !== 'function' ? value : value.bind(target);
    owner = Object.getPrototypeOf(owner);
  }
  return value;
}

/**
 * The raw `lx.automation()` driver of an app, for what the fixture does not
 * take: Logic and page scripts given as strings — this suite's contract
 * probes pass arguments the typings refuse on purpose — and raw page reads.
 * Prefer `t.app` everywhere else: raw calls are neither traced nor stopped
 * with the spec.
 *
 * The app is selected on every call, so a handle kept across an app reopen
 * (a profile rollback, a recovery) still reaches the running instance, and
 * an eval without its own `timeoutMs` gets the fixture's budget instead of
 * the driver's 5 s.
 */
export function rawApp(appId: string = SHOWCASE_APP_ID): LxAppDriver {
  const withBudget = <T extends { timeoutMs?: number }>(options: T): T => ({ timeoutMs: EVAL_BUDGET_MS, ...options });
  return new Proxy({} as LxAppDriver, {
    get(_, prop) {
      const driver = lx.automation().lxapp(appId);
      if (prop === 'eval') {
        return (options: Parameters<LxAppDriver['eval']>[0]) => driver.eval(withBudget(options));
      }
      if (prop === 'page') {
        const page = driver.page;
        return new Proxy(page, {
          get(target, prop) {
            if (prop === 'eval') {
              return (options: Parameters<typeof page.eval>[0]) => target.eval(withBudget(options));
            }
            return member(target, prop);
          },
        });
      }
      return member(driver, prop);
    },
  });
}

export function showcaseApp(): LxAppDriver {
  return rawApp(SHOWCASE_APP_ID);
}
