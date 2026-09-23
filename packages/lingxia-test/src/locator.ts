import type { PageQueryResult } from "@lingxia/types/automation";
import { AssertionError } from "./expect.js";
import { cssEscape, formatValue } from "./format.js";
import { ActionDeadline, errorCode, isElementRefusal, isPreDispatchPageError, isTransientPageError } from "./deadline.js";
import { displayLocation } from "./ids.js";
import type {
  ExpectOptions,
  Locator,
  LocatorOptions,
  LocatorState,
  LocatorWaitOptions,
  SourceLocation,
} from "./types.js";
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
  eval?(options: { script: string; timeoutMs?: number; page?: string }): Promise<unknown>;
  query(options: {
    css: string;
    page?: string;
    all?: boolean;
    index?: number;
  }): Promise<QueryMatch>;
  click(options: { css: string; page?: string; index?: number }): Promise<void>;
  fill(options: { css: string; text: string; page?: string; index?: number }): Promise<void>;
  press?(options: { css: string; key: string; page?: string; index?: number }): Promise<void>;
  type(options: { css: string; text: string; page?: string; index?: number }): Promise<void>;
}

export type Guard = <T>(op: () => T | Promise<T>) => Promise<T>;
/** Records one locator action in the report's trace. */
export type ActionRecorder = <T>(verb: string, detail: string, op: () => Promise<T>) => Promise<T>;
/** Milliseconds left in the spec (or cleanup) budget; bounds every action. */
export type BudgetRoom = () => number;

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
    private readonly options: LocatorOptions = {},
    private readonly room: BudgetRoom = () => Number.POSITIVE_INFINITY,
  ) {
    this.selector = selector;
    this.options = { ...options };
    if (!selector.trim()) throw new TypeError("Locator selector must not be empty");
    if (options.index !== undefined && (!Number.isInteger(options.index) || options.index < 0)) {
      throw new TypeError("Locator index must be a non-negative integer");
    }
  }

  nth(index: number): Locator {
    return new PageLocator(this.page, this.guard, this.record, this.selector, this.location, { ...this.options, index }, this.room);
  }

  async press(key: string, options?: ExpectOptions): Promise<void> {
    if (!this.page.press) throw new Error("This page driver does not support press");
    await this.act("press", options, (css, index) => this.page.press!({ ...this.options, css, index, key }));
  }

  async click(options?: ExpectOptions): Promise<void> {
    await this.act("click", options, (css, index) => this.page.click({ ...this.options, css, index }));
  }

  async fill(text: string, options?: ExpectOptions): Promise<void> {
    await this.act("fill", options, (css, index) => this.page.fill({ ...this.options, css, text, index }));
  }

  async type(text: string, options?: ExpectOptions): Promise<void> {
    await this.act("type", options, (css, index) => this.page.type({ ...this.options, css, text, index }));
  }

  async waitFor(options?: LocatorWaitOptions): Promise<void> {
    const state: LocatorState = options?.state ?? "visible";
    if (!["attached", "detached", "visible", "hidden"].includes(state)) {
      throw new TypeError(`Unknown locator state: ${String(state)}`);
    }
    const timeout = options?.timeout ?? DEFAULT_ACTION_TIMEOUT_MS;
    const interval = options?.interval ?? DEFAULT_POLL_INTERVAL_MS;
    if (!Number.isFinite(timeout) || timeout <= 0 || !Number.isFinite(interval) || interval <= 0) {
      throw new TypeError("Wait timeout and interval must be positive finite numbers");
    }
    const deadline = new ActionDeadline(timeout, this.room());
    await this.record("page.waitFor", `${this.target()} ${state}`, async () => {
      let reason = "";
      while (true) {
        try {
          const resolved = await this.resolve(deadline, "waitFor");
          if (reachedState(resolved, state)) return;
          reason = this.missText(resolved);
        } catch (error) {
          // A page mid-transition is not an answer yet; anything else is.
          if (!isTransientPageError(error)) throw error;
          reason = transientReason(error);
        }
        if (deadline.expired()) break;
        await sleep(Math.min(interval, Math.max(1, deadline.remaining())));
      }
      throw new AssertionError("waitFor", reason, state, [
        `Timed out after ${deadline.elapsed()}ms waiting for ${formatValue(this.selector)} to be ${state}.`,
        reason,
        deadline.clampNote(),
        `at ${this.where()}`,
      ].filter(Boolean).join("\n"));
    });
  }

  async query(): Promise<PageQueryResult> {
    return this.guard(() => this.page.query({ ...this.options, css: this.selector })) as Promise<PageQueryResult>;
  }

  /**
   * One read of the locator's matches. With a deadline the query is raced
   * against what is left of it, so a query that never returns fails `verb`.
   */
  async resolve(deadline?: ActionDeadline, verb = "resolve"): Promise<LocatorResolve> {
    const query = () => this.page.query({ page: this.options.page, css: this.selector, all: true });
    const all = await this.guard(() =>
      deadline ? deadline.call("page.query", query, () => this.callContext(verb)) : query(),
    );
    const matches = Array.isArray(all.items) ? all.items : all.exists ? [all] : [];
    const selected = this.options.index === undefined ? undefined : matches[this.options.index];
    const items: QueryMatch[] = this.options.index === undefined ? matches : selected ? [{ ...selected, index: this.options.index }] : [];
    const count = this.options.index === undefined ? (all.count ?? items.length) : items.length;
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
    if (count > 1) {
      return { count, visibleCount, attached: true, visible: visibleCount > 0,
        text: items.map(item => item.text ?? "").join("\n"), value: null, index: 0, kind: "many" };
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
      return `locator ${formatValue(this.selector)} resolved to ${resolved.count} matches`;
    }
    return `locator ${formatValue(this.selector)} resolved to a visible element`;
  }

  private target(): string {
    return `${this.options.page ? this.options.page + " " : ""}${this.selector}${this.options.index === undefined ? "" : ` [${this.options.index}]`}`;
  }

  private where(): string {
    return displayLocation(this.location.source, this.location.line, this.location.column);
  }

  private callContext(verb: string): string {
    return `while trying to ${verb} ${formatValue(this.selector)}\nat ${this.where()}`;
  }

  private async actionability(index: number, verb: string, deadline: ActionDeadline): Promise<true | string> {
    if (!this.page.eval) return true;
    const script = `(() => {
      const el = document.querySelectorAll(${JSON.stringify(this.selector)})[${index}];
      if (!el || !el.isConnected) return "element detached";
      el.scrollIntoView({block:"center", inline:"center", behavior:"instant"});
      if (el.matches(":disabled") || el.closest('[aria-disabled="true"]')) return "element is disabled";
      if ((${JSON.stringify(verb)} === "fill" || ${JSON.stringify(verb)} === "type") && (el.readOnly || el.getAttribute("aria-readonly") === "true")) return "element is readonly";
      const r = el.getBoundingClientRect();
      const hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
      return hit && (hit === el || el.contains(hit)) ? true : "element is obscured";
    })()`;
    const result = await this.guard(() => deadline.call("page.eval (actionability)",
      () => this.page.eval!({ page: this.options.page, script }), () => this.callContext(verb)));
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
    const deadline = new ActionDeadline(timeout, this.room());
    const context = () => this.callContext(verb);
    await this.record(`page.${verb}`, this.target(), async () => {
      let last: LocatorResolve | undefined;
      let previousRect: string | undefined;
      let reason = "not attached";
      // The coded driver rejection behind `reason`, if one is: a timeout it
      // caused reports its code, so `spec.fail({ expected: { code } })` and
      // the failure line can name it.
      let cause: unknown;
      while (!deadline.expired()) {
        try {
          last = await this.resolve(deadline, verb);
          reason = this.missText(last);
          cause = undefined;
          if (last.kind === "hidden" && last.count === 1 && this.page.eval) {
            const script = `document.querySelectorAll(${JSON.stringify(this.selector)})[${last.index}]?.scrollIntoView({block:"center", inline:"center", behavior:"instant"})`;
            await this.guard(() => deadline.call("page.eval (scrollIntoView)",
              () => this.page.eval!({ page: this.options.page, script }), context));
          }
          if (last.kind === "unique") {
            const rect = JSON.stringify(last.rect);
            const stable = last.rect === undefined || previousRect === rect;
            previousRect = rect;
            if (last.enabled === false) reason = "element is disabled";
            else if ((verb === "fill" || verb === "type") && last.editable === false) reason = "element is not editable";
            else if (!stable) reason = "element is moving";
            else {
              const check = await this.actionability(last.index, verb, deadline);
              if (check === true) {
                try {
                  const index = last.index;
                  await this.guard(() => deadline.call(`page.${verb}`, () => run(this.selector, index), context));
                  return;
                } catch (error) {
                  // These native errors are raised before input dispatch. A transport
                  // failure is ambiguous and must never resubmit an action.
                  if (!isElementRefusal(error) && !isPreDispatchPageError(error)) {
                    throw new DispatchFailure(error);
                  }
                  reason = error instanceof Error ? error.message : String(error);
                  cause = error;
                  previousRect = undefined;
                }
              }
              if (check !== true) reason = check;
            }
          } else { previousRect = undefined; }
        } catch (error) {
          // The page is being replaced (navigation in flight): no read or
          // dispatch reached it, so wait for the new page within the budget.
          if (error instanceof DispatchFailure) throw error.error;
          if (!isTransientPageError(error)) throw error;
          reason = transientReason(error);
          cause = error;
          previousRect = undefined;
        }
        await sleep(Math.min(interval, Math.max(1, deadline.remaining())));
      }
      throw withCause(new AssertionError(verb, reason, "stable, enabled, unobscured element", [
        `Timed out after ${deadline.elapsed()}ms waiting to ${verb} ${formatValue(this.selector)}.`,
        reason,
        deadline.clampNote(),
        `at ${this.where()}`,
      ].filter(Boolean).join("\n")), cause);
    });
  }
}

/** Carry the `code`/`data` of the driver rejection that kept an action from running. */
function withCause(error: AssertionError, cause: unknown): AssertionError {
  const code = errorCode(cause);
  if (code === undefined) return error;
  const data = (cause as { data?: unknown }).data;
  return Object.assign(error, { code, ...(data === undefined ? {} : { data }) });
}

/** A dispatch error that must propagate as is, never be retried. */
class DispatchFailure {
  constructor(readonly error: unknown) {}
}

function transientReason(error: unknown): string {
  return `page not ready: ${error instanceof Error ? error.message : String(error)}`;
}

/** `many` never satisfies `attached`/`visible`: the match is ambiguous. */
function reachedState(resolved: LocatorResolve, state: LocatorState): boolean {
  switch (state) {
    case "attached":
      return resolved.kind === "unique" || resolved.kind === "hidden";
    case "visible":
      return resolved.kind === "unique";
    case "hidden":
      return resolved.visibleCount === 0;
    case "detached":
      return resolved.kind === "nothing";
  }
}

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}
