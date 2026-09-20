import { AssertionError } from "./expect.js";
import { cssEscape, formatValue } from "./format.js";
import { displayLocation } from "./ids.js";
import type { ExpectOptions, Locator, SourceLocation } from "./types.js";
import {
  DEFAULT_ACTION_TIMEOUT_MS,
  DEFAULT_POLL_INTERVAL_MS,
} from "./version.js";

export interface QueryMatch {
  exists: boolean;
  count: number;
  index?: number;
  visible?: boolean;
  text?: string;
  value?: string | null;
  enabled?: boolean;
  editable?: boolean;
  rect?: { left: number; top: number; width: number; height: number };
  items?: QueryMatch[];
}

export interface PageLike {
  eval?(options: { script: string; timeoutMs?: number }): Promise<unknown>;
  query(options: {
    css: string;
    all?: boolean;
    index?: number;
  }): Promise<QueryMatch>;
  click(options: { css: string; index?: number }): Promise<void>;
  fill(options: { css: string; text: string; index?: number }): Promise<void>;
  type(options: { css: string; text: string; index?: number }): Promise<void>;
}

export type Guard = <T>(op: () => T | Promise<T>) => Promise<T>;
/** Records one locator action in the report's trace. */
export type ActionRecorder = <T>(verb: string, detail: string, op: () => Promise<T>) => Promise<T>;

export interface LocatorResolve {
  count: number;
  visibleCount: number;
  attached: boolean;
  visible: boolean;
  text: string;
  value: string | null;
  index: number;
  enabled?: boolean;
  editable?: boolean;
  rect?: QueryMatch["rect"];
  kind: "nothing" | "hidden" | "unique" | "many";
}

export function testIdSelector(id: string): string {
  return `[data-testid="${cssEscape(id)}"]`;
}

export class PageLocator implements Locator {
  readonly selector: string;

  constructor(
    private readonly page: PageLike,
    private readonly guard: Guard,
    private readonly record: ActionRecorder,
    selector: string,
    private readonly location: SourceLocation,
  ) {
    this.selector = selector;
  }

  async click(options?: ExpectOptions): Promise<void> {
    await this.act("click", options, (css, index) => this.page.click({ css, index }));
  }

  async fill(text: string, options?: ExpectOptions): Promise<void> {
    await this.act("fill", options, (css, index) => this.page.fill({ css, text, index }));
  }

  async type(text: string, options?: ExpectOptions): Promise<void> {
    await this.act("type", options, (css, index) => this.page.type({ css, text, index }));
  }

  async query(): Promise<QueryMatch> {
    return this.guard(() => this.page.query({ css: this.selector }));
  }

  async resolve(): Promise<LocatorResolve> {
    const all = await this.guard(() =>
      this.page.query({ css: this.selector, all: true }),
    );
    const items = Array.isArray(all.items) ? all.items : all.exists ? [all] : [];
    const count = typeof all.count === "number" ? all.count : items.length;
    const visibleItems = items.filter((item) => item.visible);
    const visibleCount = visibleItems.length;
    if (count === 0) {
      return {
        count: 0,
        visibleCount: 0,
        attached: false,
        visible: false,
        text: "",
        value: null,
        index: 0,
        kind: "nothing",
      };
    }
    if (visibleCount === 0) {
      return {
        count,
        visibleCount: 0,
        attached: true,
        visible: false,
        text: items[0]?.text ?? "",
        value: items[0]?.value ?? null,
        index: items[0]?.index ?? 0,
        enabled: items[0]?.enabled, editable: items[0]?.editable,
        kind: "hidden",
      };
    }
    if (visibleCount > 1) {
      return {
        count,
        visibleCount,
        attached: true,
        visible: true,
        text: visibleItems.map((item) => item.text ?? "").join("\n"),
        value: visibleItems[0]?.value ?? null,
        index: visibleItems[0]?.index ?? 0,
        kind: "many",
      };
    }
    const unique = visibleItems[0]!;
    return {
      count,
      visibleCount: 1,
      attached: true,
      visible: true,
      text: unique.text ?? "",
      value: unique.value ?? null,
      index: unique.index ?? items.indexOf(unique),
      enabled: unique.enabled, editable: unique.editable, rect: unique.rect,
      kind: "unique",
    };
  }

  missText(resolved: LocatorResolve): string {
    if (resolved.kind === "nothing") {
      return `locator ${formatValue(this.selector)} resolved to nothing`;
    }
    if (resolved.kind === "hidden") {
      return `locator ${formatValue(this.selector)} resolved to hidden`;
    }
    if (resolved.kind === "many") {
      return `locator ${formatValue(this.selector)} resolved to ${resolved.visibleCount} matches`;
    }
    return `locator ${formatValue(this.selector)} resolved to a visible element`;
  }

  private async actionability(index: number, verb: string): Promise<true | string> {
    if (!this.page.eval) return true;
    const result = await this.guard(() => this.page.eval!({ script: `(() => {
      const el = document.querySelectorAll(${JSON.stringify(this.selector)})[${index}];
      if (!el || !el.isConnected) return "element detached";
      el.scrollIntoView({block:"center", inline:"center", behavior:"instant"});
      if (el.matches(":disabled") || el.closest('[aria-disabled="true"]')) return "element is disabled";
      if (${JSON.stringify(verb)} !== "click" && (el.readOnly || el.getAttribute("aria-readonly") === "true")) return "element is readonly";
      const r = el.getBoundingClientRect();
      const hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
      return hit && (hit === el || el.contains(hit)) ? true : "element is obscured";
    })()` }));
    return result === true ? true : String(result ?? "invalid actionability response");
  }

  private async act(
    verb: string,
    options: ExpectOptions | undefined,
    run: (css: string, index: number) => Promise<void>,
  ): Promise<void> {
    const timeout = options?.timeout ?? DEFAULT_ACTION_TIMEOUT_MS;
    const interval = options?.interval ?? DEFAULT_POLL_INTERVAL_MS;
    if (!Number.isFinite(timeout) || timeout <= 0 || !Number.isFinite(interval) || interval <= 0) {
      throw new TypeError("Action timeout and interval must be positive finite numbers");
    }
    await this.record(`page.${verb}`, this.selector, async () => {
      const started = Date.now();
      let last: LocatorResolve | undefined;
      let previousRect: string | undefined;
      let reason = "not attached";
      while (Date.now() - started < timeout) {
        last = await this.resolve();
        reason = this.missText(last);
        if (last.kind === "hidden" && last.count === 1 && this.page.eval) {
          await this.guard(() => this.page.eval!({ script: `document.querySelectorAll(${JSON.stringify(this.selector)})[${last!.index}]?.scrollIntoView({block:"center", inline:"center", behavior:"instant"})` }));
        }
        if (last.kind === "unique") {
          const rect = JSON.stringify(last.rect);
          const stable = last.rect === undefined || previousRect === rect;
          previousRect = rect;
          if (last.enabled === false) reason = "element is disabled";
          else if (verb !== "click" && last.editable === false) reason = "element is not editable";
          else if (!stable) reason = "element is moving";
          else {
            const check = await this.actionability(last.index, verb);
            if (check === true) {
              try {
                await this.guard(() => run(this.selector, last!.index));
                return;
              } catch (error) {
                // These native errors are raised before input dispatch. A transport
                // failure is ambiguous and must never resubmit an action.
                const message = error instanceof Error ? error.message : "";
                if (!/^Element (?:not found|not interactable):/.test(message)) throw error;
                reason = message;
                previousRect = undefined;
              }
            }
            if (check !== true) reason = check;
          }
        } else { previousRect = undefined; }
        await sleep(Math.min(interval, Math.max(1, timeout - (Date.now() - started))));
      }
      const where = displayLocation(this.location.source, this.location.line, this.location.column);
      throw new AssertionError(verb, reason, "stable, enabled, unobscured element",
        `Timed out after ${Date.now() - started}ms waiting to ${verb} ${formatValue(this.selector)}.\n${reason}\nat ${where}`);
    });
  }
}

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}
