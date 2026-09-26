import type { LxAppDriver } from '@lingxia/types/automation';

export const SHOWCASE_APP_ID = 'lingxia-showcase';

/** What a raw eval may take: the fixture gives its evals a third of a 30 s spec. */
const EVAL_BUDGET_MS = 10_000;

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
          get(target, member) {
            if (member === 'eval') {
              return (options: Parameters<typeof page.eval>[0]) => target.eval(withBudget(options));
            }
            const value = Reflect.get(target, member, target);
            return typeof value === 'function' ? value.bind(target) : value;
          },
        });
      }
      const value = Reflect.get(driver, prop, driver);
      return typeof value === 'function' ? value.bind(driver) : value;
    },
  });
}

export function showcaseApp(): LxAppDriver {
  return rawApp(SHOWCASE_APP_ID);
}
