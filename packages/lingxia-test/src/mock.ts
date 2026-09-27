import type {
  LxAppDriver,
  Scenario,
  ScenarioCall,
  ScenarioCallFilter,
  ScenarioInput,
  ScenarioRuleInfo,
} from "@lingxia/types/automation";
import { truncate } from "./format.js";
import { errorText, scenarioCall, waitForNextCall, type CallCursor, type NetworkHost } from "./network.js";
import type { ScenarioCallTarget, TestScenario, WaitForCallOptions } from "./types.js";
import type { ScenarioReport } from "./report-types.js";
import { runnerSetTimeout } from "./pending.js";

/**
 * The scenario one spec installed with `t.app.mock.use()`. Installing
 * another replaces it (the host does the same for its run), and the runner
 * removes it when the spec ends.
 */
export class ScenarioScope {
  current: { raw: Scenario; label: string } | undefined;

  track(raw: Scenario, label: string): void {
    this.current = { raw, label };
  }

  /** Remove the scenario; one already removed counts as removed. */
  async reclaim(): Promise<void> {
    const current = this.current;
    if (!current) return;
    // Raw handle: the fixture is closed by now.
    try {
      await current.raw.unroute();
    } catch (error) {
      throw new Error(`mock scenario ${current.label} was not removed: ${errorText(error)}`);
    }
    if (this.current === current) this.current = undefined;
  }

  /**
   * What a failed spec reports about its scenario: each rule's hits and the
   * Function calls the companion saw. Never throws.
   */
  async report(budgetMs: number): Promise<{ scenario: ScenarioReport; functionCalls: ScenarioCall[] } | undefined> {
    const current = this.current;
    if (!current) return undefined;
    let rules: ScenarioRuleInfo[] = [];
    let calls: ScenarioCall[] = [];
    try {
      rules = [...current.raw.rules];
      calls = await Promise.race([
        current.raw.calls(),
        new Promise<ScenarioCall[]>((resolve) => { runnerSetTimeout(() => resolve([]), budgetMs); }),
      ]);
    } catch {
      // Evidence never replaces the failure.
    }
    return {
      scenario: {
        label: current.label,
        name: current.raw.name,
        variant: current.raw.variant,
        rules: rules.map((rule) => ({
          index: rule.index,
          target: rule.target,
          kind: rule.kind,
          hits: rule.hits ?? calls.filter((call) => call.rule === rule.index).length,
        })),
      },
      functionCalls: calls.filter((call) => call.kind === "function"),
    };
  }
}

/** `name:variant`, `scenario` standing in for a missing name. */
export function scenarioLabel(definition: ScenarioInput, variant: string | undefined): string {
  const record = definition as { name?: unknown };
  const name = typeof record.name === "string" ? record.name : "scenario";
  return variant ? `${name}:${variant}` : name;
}

function describeScenario(definition: ScenarioInput, variant: string | undefined): string {
  const record = definition as { rules?: unknown; variants?: Record<string, { rules?: unknown }> };
  const shared = Array.isArray(record.rules) ? record.rules.length : 0;
  const own = variant && Array.isArray(record.variants?.[variant]?.rules) ? record.variants[variant].rules!.length : 0;
  const count = shared + (own as number);
  return truncate(`${scenarioLabel(definition, variant)} (${count} rule${count === 1 ? "" : "s"})`, 80);
}

function describeTarget(target: ScenarioCallTarget): string {
  if ("http" in target) return target.http;
  if ("function" in target) return `function ${target.function}`;
  return `rule ${target.rule}`;
}

function validTarget(target: unknown): target is ScenarioCallTarget {
  if (!target || typeof target !== "object") return false;
  const record = target as Record<string, unknown>;
  return typeof record.http === "string" || typeof record.function === "string" || typeof record.rule === "number";
}

/**
 * `t.app.mock.use(file, variant?)`: the host installs the scenario for the
 * run, on top of the mock selection; the fixture removes it when the spec
 * ends and traces every call.
 */
export function installScenario(
  resolve: () => LxAppDriver,
  host: NetworkHost,
  scope: ScenarioScope,
  definition: ScenarioInput,
  variant?: string,
): Promise<TestScenario> {
  return host.act("mock.use", describeScenario(definition, variant), async () => {
    const raw = variant === undefined
      ? await resolve().mock.use(definition)
      : await resolve().mock.use(definition, variant);
    const label = scenarioLabel(definition, variant);
    scope.track(raw, label);
    const read = async (filter?: ScenarioCallFilter) => (await raw.calls(filter)).map(scenarioCall);
    const cursors = new Map<string, CallCursor>();
    const wrapped: TestScenario = {
      get name() { return raw.name; },
      get variant() { return raw.variant; },
      get rules() { return raw.rules; },
      calls: (filter?: ScenarioCallFilter) =>
        host.act("scenario.calls", filter ? JSON.stringify(filter) : label, () => read(filter)),
      waitForCall: (target: ScenarioCallTarget, options?: WaitForCallOptions) => {
        if (!validTarget(target)) {
          throw new TypeError("scenario.waitForCall needs { http: 'METHOD url' }, { function: 'name' } or { rule: n }");
        }
        const key = JSON.stringify(target);
        let cursor = cursors.get(key);
        if (!cursor) cursors.set(key, cursor = { after: 0, taken: 0 });
        const after = async (seq: number) => {
          const window = await raw.callsAfter(target, seq);
          return { calls: window.calls.map(scenarioCall), droppedThrough: window.droppedThrough };
        };
        return waitForNextCall(host, "scenario.waitForCall", `scenario ${label}: ${describeTarget(target)}`,
          after, () => read(), cursor, options);
      },
      remove: () =>
        host.act("scenario.remove", label, async () => {
          if (scope.current?.raw === raw) scope.current = undefined;
          await raw.unroute();
        }),
    };
    return wrapped;
  });
}

/**
 * Each spec starts with fresh mock handler state: the app evaluates
 * `mocks/index.ts` again on its next call. Returns why a companion could
 * not do the same for the run's Functions, once per run (`reasons` holds
 * what was already said).
 */
export async function resetMocks(driver: LxAppDriver, reasons: Set<string>): Promise<string | undefined> {
  const mock = (driver as Partial<LxAppDriver>).mock;
  // A host that predates mocks has nothing to reset.
  if (!mock || typeof mock.reset !== "function") return undefined;
  const result = await mock.reset();
  const reason = result.function && result.function.reset === false
    ? result.function.reason ?? "the companion cannot start its handler state over"
    : undefined;
  if (reason === undefined || reasons.has(reason)) return undefined;
  reasons.add(reason);
  return `mock handler state of the Functions is not reset per spec: ${reason}`;
}
