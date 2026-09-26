import type {
  LxAppDriver,
  Scenario,
  ScenarioCall,
  ScenarioCallFilter,
  ScenarioInput,
  ScenarioRuleInfo,
} from "@lingxia/types/automation";
import { truncate } from "./format.js";
import { scenarioCall, waitForNextCall, type NetworkHost } from "./network.js";
import type { ScenarioCallTarget, ScenarioReport, TestScenario, WaitForCallOptions } from "./types.js";

/**
 * The scenario one spec installed with `t.app.scenario()`. Installing
 * another replaces it (the host does the same for its run), and it is
 * removed when the spec ends.
 */
export class ScenarioScope {
  current: { raw: Scenario; label: string } | undefined;
  private cleanupRegistered = false;

  track(host: NetworkHost, raw: Scenario, label: string): void {
    this.current = { raw, label };
    if (this.cleanupRegistered) return;
    this.cleanupRegistered = true;
    host.defer(async () => {
      const current = this.current;
      this.current = undefined;
      // Raw handle: cleanup runs after the fixture closed its action guard;
      // the run's end removes whatever is left.
      try { await current?.raw.unroute(); } catch { /* the run end clears it */ }
    });
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
        new Promise<ScenarioCall[]>((resolve) => setTimeout(() => resolve([]), budgetMs)),
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
 * `t.app.scenario(file, variant?)`: the host installs it for the run; the
 * fixture removes it when the spec ends and traces every call.
 */
export function installScenario(
  resolve: () => LxAppDriver,
  host: NetworkHost,
  scope: ScenarioScope,
  definition: ScenarioInput,
  variant?: string,
): Promise<TestScenario> {
  return host.act("scenario", describeScenario(definition, variant), async () => {
    const raw = variant === undefined
      ? await resolve().scenario(definition)
      : await resolve().scenario(definition, variant);
    const label = scenarioLabel(definition, variant);
    scope.track(host, raw, label);
    const read = async (filter?: ScenarioCallFilter) => (await raw.calls(filter)).map(scenarioCall);
    const cursors = new Map<string, { taken: number }>();
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
        if (!cursor) cursors.set(key, cursor = { taken: 0 });
        return waitForNextCall(host, "scenario.waitForCall", `scenario ${label}: ${describeTarget(target)}`,
          () => read(target), () => read(), cursor, options);
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
