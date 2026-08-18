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
  items?: QueryMatch[];
}

export interface PageLike {
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
export type Record = <T>(verb: string, detail: string, op: () => Promise<T>) => Promise<T>;

export interface LocatorResolve {
  count: number;
  visibleCount: number;
  attached: boolean;
  visible: boolean;
  text: string;
  value: string | null;
  index: number;
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
    private readonly record: Record,
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

  private async act(
    verb: string,
    options: ExpectOptions | undefined,
    run: (css: string, index: number) => Promise<void>,
  ): Promise<void> {
    const timeout = options?.timeout ?? DEFAULT_ACTION_TIMEOUT_MS;
    const interval = options?.interval ?? DEFAULT_POLL_INTERVAL_MS;
    const started = Date.now();
    let last: LocatorResolve | undefined;
    while (true) {
      last = await this.resolve();
      if (last.kind === "unique") {
        await this.record(`page.${verb}`, this.selector, () =>
          this.guard(() => run(this.selector, last!.index)),
        );
        return;
      }
      if (Date.now() - started >= timeout) break;
      await sleep(interval);
    }
    const resolved = last ?? await this.resolve();
    const duration = Date.now() - started;
    const where = displayLocation(this.location.source, this.location.line, this.location.column);
    throw new AssertionError(
      verb,
      resolved.kind,
      "visible unique element",
      [
        `Timed out after ${duration}ms waiting to ${verb} ${formatValue(this.selector)}.`,
        this.missText(resolved),
        `at ${where}`,
      ].join("\n"),
    );
  }
}

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}
