import { cssEscape, formatValue } from "./format.js";
import {
  ActionDeadline,
  TimeoutError,
  errorCode,
  isElementRefusal,
  isPreDispatchPageError,
  isTransientPageError,
  isTransientTransportError,
} from "./deadline.js";
import { resolveHost } from "./host.js";
import { callerLocation, displayLocation } from "./ids.js";
import type {
  ActionOptions,
  InputOptions,
  Locator,
  LocatorRole,
  LocatorFilterOptions,
  LocatorState,
  LocatorWaitOptions,
  SourceLocation,
} from "./types.js";
import type {
  PageVisibility,
} from "./report-types.js";
import {
  DEFAULT_ACTION_TIMEOUT_MS,
  DEFAULT_POLL_INTERVAL_MS,
} from "./version.js";
import { runnerSetTimeout } from "./pending.js";

export interface QueryMatch {
  exists: boolean;
  count: number;
  index?: number;
  visible?: boolean;
  /** Rendered and intersecting the viewport. */
  inViewport?: boolean;
  /** The `aria-label`; on a GPU page, the accessible name. */
  ariaLabel?: string | null;
  /** `gpu` for a native View page; absent for a WebView page. */
  renderer?: "gpu";
  text?: string;
  value?: string | null;
  enabled?: boolean;
  editable?: boolean;
  /** Holds keyboard focus (`document.activeElement` on a WebView page). */
  focused?: boolean;
  /** A checkbox, radio or `aria-checked` state; null when the element has none. */
  /** `"mixed"`: partly checked (a GPU page's `indeterminate` checkbox). */
  checked?: boolean | "mixed" | null;
  /** `aria-pressed`; null when absent. */
  pressed?: boolean | null;
  /** An `<option>`'s or `aria-selected` state; null when the element has none. */
  selected?: boolean | null;
  /** `aria-expanded`, or a `<summary>`'s `<details>` open state; null when the element has none. */
  expanded?: boolean | null;
  /** `aria-busy`; null when absent. */
  busy?: boolean | null;
  /** On a GPU page: `data-testid`, the authored `data-*` props and `aria-invalid`/`-required`/`-describedby`/`-controls`/`-autocomplete`. */
  attributes?: Record<string, string>;
  rect?: {
    left: number;
    top: number;
    width: number;
    height: number;
    viewportWidth?: number;
    viewportHeight?: number;
  };
  /** On a GPU page: the part of `rect` a finger reaches, past scroll clips and host bars; null when none. */
  visibleRect?: { left: number; top: number; width: number; height: number } | null;
  /** On a GPU page: the box a finger reaches when nothing else lies under it. */
  targetRect?: { left: number; top: number; width: number; height: number };
  items?: QueryMatch[];
}

/** The ARIA states a match reports; a host that predates them reports none. */
function states(match: QueryMatch | undefined): Pick<LocatorResolve, "checked" | "pressed" | "selected" | "expanded" | "busy"> {
  return {
    checked: match?.checked ?? null,
    pressed: match?.pressed ?? null,
    selected: match?.selected ?? null,
    expanded: match?.expanded ?? null,
    busy: match?.busy ?? null,
  };
}

export interface PageLike {
  eval?(options: { script: string; timeoutMs?: number; page?: string }): Promise<unknown>;
  query(options: {
    css: string;
    page?: string;
    all?: boolean;
    index?: number;
  }): Promise<QueryMatch>;
  click(options: { css: string; page?: string; index?: number; force?: boolean }): Promise<void>;
  fill(options: { css: string; text: string; page?: string; index?: number; force?: boolean }): Promise<void>;
  press?(options: { css: string; key: string; page?: string; index?: number }): Promise<void>;
  type(options: { css: string; text: string; page?: string; index?: number }): Promise<void>;
  selectOption?(options: { css: string; value: string; page?: string; index?: number }): Promise<void>;
  dblclick?(options: { css: string; page?: string; index?: number }): Promise<void>;
  blur?(options: { css: string; page?: string; index?: number }): Promise<void>;
  semanticAction?(options: { css: string; page?: string; index?: number; name: string }): Promise<void>;
  /** Bring a match on screen; GPU pages need it before an action. */
  scrollTo?(options: { css: string; page?: string; index?: number }): Promise<void>;
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
  /** The unique match is rendered and intersects the viewport. */
  inViewport: boolean;
  text: string;
  value: string | null;
  index: number;
  enabled?: boolean;
  editable?: boolean;
  focused?: boolean;
  /** `"mixed"`: partly checked (a GPU page's `indeterminate` checkbox). */
  checked?: boolean | "mixed" | null;
  pressed?: boolean | null;
  selected?: boolean | null;
  expanded?: boolean | null;
  busy?: boolean | null;
  rect?: QueryMatch["rect"];
  visibleRect?: QueryMatch["visibleRect"];
  targetRect?: QueryMatch["targetRect"];
  renderer?: "gpu";
  /** Attribute values read for the single match, by name (`null`: absent). */
  attributes?: Record<string, string | null>;
  kind: "nothing" | "hidden" | "unique" | "many";
}

/** The page a locator reads (a live instance id; omitted: the current page) and its pick. */
export interface LocatorTarget {
  page?: string;
  /** Zero-based match index; omitted, the locator needs a unique match. */
  index?: number;
}

/** Narrowing applied to the raw matches before `nth`/`first`/`last` picks one. */
export interface LocatorRefine {
  hasText?: string | RegExp;
  /** The accessible name (`aria-label`, else the text), whole and exact, or a pattern. */
  name?: string | RegExp;
  /** Only matches whose selected state is this; a match with no such state counts as not selected. */
  selected?: boolean;
  /** Pick the last match (`.last()`); `LocatorTarget.index` picks from the start. */
  last?: boolean;
  /** Built by `Locator.getByRole`: the selector is a descendant chain. */
  scoped?: boolean;
}

/** Marks a locator across module copies, so `expect(locator)` retries it. */
export const LOCATOR_BRAND = Symbol.for("lingxia.test.locator");

export function testIdSelector(id: string): string {
  return `[data-testid="${cssEscape(id)}"]`;
}

/**
 * The elements that carry `role`, natively or by ARIA, as one selector both
 * renderers answer: a WebView by tag and attribute, a GPU page by the role
 * its semantic projection gives each node. A WebView `<input>` must state
 * its `type` to count as a textbox.
 */
export const ROLE_SELECTORS: Readonly<Record<LocatorRole, string>> = {
  button: 'button, [role="button"], input[type="button"], input[type="submit"]',
  link: 'a[href], [role="link"]',
  heading: 'h1, h2, h3, h4, h5, h6, [role="heading"]',
  textbox: 'input[type="text"], input[type="search"], input[type="email"], input[type="tel"], input[type="url"], input[type="password"], input[type="number"], textarea, [role="textbox"]',
  checkbox: 'input[type="checkbox"], [role="checkbox"]',
  switch: '[role="switch"]',
  radio: 'input[type="radio"], [role="radio"]',
  radiogroup: '[role="radiogroup"]',
  slider: 'input[type="range"], [role="slider"]',
  img: 'img, [role="img"]',
  list: 'ul, ol, [role="list"]',
  listitem: 'li, [role="listitem"]',
  dialog: 'dialog, [role="dialog"]',
  alert: '[role="alert"]',
  status: '[role="status"]',
  combobox: 'select, [role="combobox"]',
  menu: '[role="menu"]',
  menuitem: '[role="menuitem"]',
  progressbar: 'progress, [role="progressbar"]',
  alertdialog: '[role="alertdialog"]',
  group: '[role="group"]',
  tooltip: '[role="tooltip"]',
  tablist: '[role="tablist"]',
  tab: '[role="tab"]',
  tabpanel: '[role="tabpanel"]',
  toolbar: '[role="toolbar"]',
  tree: '[role="tree"]',
  treeitem: '[role="treeitem"]',
  listbox: '[role="listbox"]',
  option: '[role="option"]',
  menuitemcheckbox: '[role="menuitemcheckbox"]',
  menuitemradio: '[role="menuitemradio"]',
};

/** The top-level parts of a selector list; commas inside quotes or brackets stay. */
function selectorParts(selector: string): string[] {
  const parts: string[] = [];
  let start = 0, quote: string | undefined, escaped = false, bracket = false;
  for (let at = 0; at < selector.length; at++) {
    const c = selector[at];
    if (escaped) escaped = false;
    else if (quote) { if (c === "\\") escaped = true; else if (c === quote) quote = undefined; }
    else if (c === '"' || c === "'") quote = c;
    else if (c === "[") bracket = true;
    else if (c === "]") bracket = false;
    else if (c === "," && !bracket) { parts.push(selector.slice(start, at).trim()); start = at + 1; }
  }
  parts.push(selector.slice(start).trim());
  return parts.filter(Boolean);
}

/** `inner` inside any match of `outer`, as one selector list (descendant combinator). */
export function descendantSelector(outer: string, inner: string): string {
  return selectorParts(outer).flatMap((scope) => selectorParts(inner).map((part) => `${scope} ${part}`)).join(", ");
}

// A GPU host that predates descendant chains lists no such form in its refusal.
const SCOPED_GPU_QUERIES = "descendant chains";

/** `getByRole`'s options as a refinement; an option it does not know is an error, never ignored. */
export function roleRefine(options: unknown): Pick<LocatorRefine, "name" | "selected"> {
  if (options === undefined) return {};
  if (options === null || typeof options !== "object") throw new TypeError("getByRole() options must be an object");
  const { name, selected, ...unknown } = options as Record<string, unknown>;
  const extra = Object.keys(unknown);
  if (extra.length) throw new TypeError(`getByRole() takes name and selected, not ${extra.join(", ")}`);
  if (name !== undefined && typeof name !== "string" && !(name instanceof RegExp)) {
    throw new TypeError("getByRole() name must be a string or RegExp");
  }
  if (selected !== undefined && typeof selected !== "boolean") throw new TypeError("getByRole() selected must be true or false");
  return { ...(name === undefined ? {} : { name }), ...(selected === undefined ? {} : { selected }) };
}

export function roleSelector(role: string): string {
  if (!Object.prototype.hasOwnProperty.call(ROLE_SELECTORS, role)) {
    throw new TypeError(`getByRole: unknown role ${JSON.stringify(role)}; known: ${Object.keys(ROLE_SELECTORS).join(", ")}`);
  }
  return ROLE_SELECTORS[role as LocatorRole];
}

export class PageLocator implements Locator {
  readonly selector: string;
  readonly [LOCATOR_BRAND] = true;

  constructor(
    private readonly page: PageLike,
    private readonly guard: Guard,
    private readonly record: ActionRecorder,
    selector: string,
    private readonly location: SourceLocation,
    private readonly options: LocatorTarget = {},
    private readonly room: BudgetRoom = () => Number.POSITIVE_INFINITY,
    private readonly refine: LocatorRefine = {},
  ) {
    this.selector = selector;
    this.options = { ...options };
    this.refine = { ...refine };
    if (!selector.trim()) throw new TypeError("Locator selector must not be empty");
    if (options.index !== undefined && (!Number.isInteger(options.index) || options.index < 0)) {
      throw new TypeError("Locator index must be a non-negative integer");
    }
  }

  nth(index: number): Locator {
    return new PageLocator(this.page, this.guard, this.record, this.selector, this.location,
      { ...this.options, index }, this.room, { ...this.refine, last: false });
  }

  first(): Locator {
    return this.nth(0);
  }

  last(): Locator {
    const { index: _index, ...options } = this.options;
    return new PageLocator(this.page, this.guard, this.record, this.selector, this.location,
      options, this.room, { ...this.refine, last: true });
  }

  getByRole(role: LocatorRole, options?: { name?: string | RegExp; selected?: boolean }): Locator {
    const refine = roleRefine(options);
    const narrowed = this.refine.last ? "last()"
      : this.options.index !== undefined ? `nth(${this.options.index})`
      : this.refine.hasText !== undefined ? "filter({ hasText })"
      : this.refine.name !== undefined ? "getByRole(..., { name })"
      : this.refine.selected !== undefined ? "getByRole(..., { selected })"
      : undefined;
    if (narrowed) {
      throw new TypeError(`getByRole() cannot scope to a locator narrowed with ${narrowed}: a scope is every match ` +
        "of its selector. Scope with view.testId(), view.css() or view.getByRole() without a name, and narrow the inner locator instead");
    }
    const frame = callerLocation();
    return new PageLocator(this.page, this.guard, this.record, descendantSelector(this.selector, roleSelector(role)),
      { source: frame.file, line: frame.line, column: frame.column }, this.options, this.room,
      { ...refine, scoped: true });
  }

  filter(options: LocatorFilterOptions): Locator {
    const hasText = options?.hasText;
    if (typeof hasText !== "string" && !(hasText instanceof RegExp)) {
      throw new TypeError("filter() needs { hasText: string | RegExp }");
    }
    return new PageLocator(this.page, this.guard, this.record, this.selector, this.location,
      this.options, this.room, { ...this.refine, hasText });
  }

  // Actions return the fixture's own call, not a wrapper around it: a call
  // the spec never awaited is then the very promise the runtime stops.
  press(key: string, options?: InputOptions): Promise<void> {
    if (!this.page.press) return Promise.reject(new Error("This page driver does not support press"));
    return this.act("press", options, (css, index) => this.page.press!({ page: this.options.page, css, index, key }), `key=${formatValue(key)}`);
  }

  click(options?: ActionOptions): Promise<void> {
    const force = options?.force === true;
    return this.act("click", options, (css, index) =>
      this.page.click({ page: this.options.page, css, index, ...(force ? { force } : {}) }));
  }

  performAction(name: string, options?: InputOptions): Promise<void> {
    if (typeof name !== "string" || !name) return Promise.reject(new TypeError("performAction() takes an action name"));
    if (!this.page.semanticAction) return Promise.reject(new Error("This page driver does not support semantic actions"));
    return this.act("performAction", options, (css, index) => this.page.semanticAction!({ page: this.options.page, css, index, name }));
  }

  dblclick(options?: ActionOptions): Promise<void> {
    if (!this.page.dblclick) return Promise.reject(new Error("This page driver does not support dblclick"));
    return this.act("dblclick", options, (css, index) => this.page.dblclick!({ page: this.options.page, css, index }));
  }

  blur(options?: InputOptions): Promise<void> {
    if (!this.page.blur) return Promise.reject(new Error("This page driver does not support blur"));
    return this.act("blur", options, (css, index) => this.page.blur!({ page: this.options.page, css, index }));
  }

  selectOption(value: string, options?: ActionOptions): Promise<void> {
    if (!this.page.selectOption) return Promise.reject(new Error("This page driver does not support selectOption"));
    return this.act("selectOption", options, (css, index) =>
      this.page.selectOption!({ page: this.options.page, css, value, index }), `value=${formatValue(value)}`);
  }

  fill(text: string, options?: ActionOptions): Promise<void> {
    const force = options?.force === true;
    return this.act("fill", options, (css, index) =>
      this.page.fill({ page: this.options.page, css, text, index, ...(force ? { force } : {}) }), this.inputDetail(text));
  }

  type(text: string, options?: InputOptions): Promise<void> {
    return this.act("type", options, (css, index) => this.page.type({ page: this.options.page, css, text, index }), this.inputDetail(text));
  }

  waitFor(options?: LocatorWaitOptions): Promise<void> {
    const state: LocatorState = options?.state ?? "visible";
    if (!["attached", "detached", "visible", "hidden", "inViewport"].includes(state)) {
      return Promise.reject(new TypeError(`Unknown locator state: ${String(state)}`));
    }
    const timeout = options?.timeout ?? DEFAULT_ACTION_TIMEOUT_MS;
    const interval = options?.interval ?? DEFAULT_POLL_INTERVAL_MS;
    if (!Number.isFinite(timeout) || timeout <= 0 || !Number.isFinite(interval) || interval <= 0) {
      return Promise.reject(new TypeError("Wait timeout and interval must be positive finite numbers"));
    }
    const deadline = new ActionDeadline(timeout, this.room());
    return this.record("page.waitFor", `${this.target()} ${state}`, async () => {
      let reason = "not observed";
      let cause: unknown;
      while (!deadline.expired()) {
        try {
          const resolved = await this.resolve(deadline, "waitFor");
          if (reachedState(resolved, state)) return;
          reason = this.missText(resolved);
          cause = undefined;
        } catch (error) {
          // A page mid-transition, or a read the transport dropped, is not an
          // answer yet; anything else is.
          if (!isTransientPageError(error) && !isTransientTransportError(error)) throw error;
          reason = transientReason(error);
          cause = error;
        }
        if (deadline.expired()) break;
        await sleep(Math.min(interval, Math.max(1, deadline.remaining())));
      }
      throw withCause(new TimeoutError([
        `Timed out after ${deadline.elapsed()}ms waiting for ${formatValue(this.selector)} to be ${state}.`,
        reason,
        deadline.clampNote(),
        `at ${this.where()}`,
      ].filter(Boolean).join("\n")), cause);
    });
  }

  count(): Promise<number> {
    return this.read("count", async () => (await this.resolve(undefined, "count")).count);
  }

  isVisible(): Promise<boolean> {
    return this.read("isVisible", async () => {
      const resolved = await this.resolve(undefined, "isVisible");
      if (resolved.kind === "many") throw new Error(`isVisible(): ${this.missText(resolved)}; narrow it with nth(), first(), last() or filter()`);
      return resolved.kind === "unique";
    });
  }

  textContent(): Promise<string> {
    return this.read("textContent", async () => (await this.single("textContent")).text);
  }

  inputValue(): Promise<string> {
    return this.read("inputValue", async () => {
      const resolved = await this.single("inputValue");
      if (resolved.value === null) throw new Error(`inputValue(): ${this.target()} has no value (not an input, textarea or select)`);
      return resolved.value;
    });
  }

  getAttribute(name: string): Promise<string | null> {
    if (typeof name !== "string" || name.length === 0) {
      return Promise.reject(new TypeError("getAttribute(name) takes an attribute name"));
    }
    return this.read("getAttribute", async () => (await this.single("getAttribute", [name])).attributes?.[name] ?? null);
  }

  boundingBox(): Promise<{ x: number; y: number; width: number; height: number }> {
    return this.read("boundingBox", async () => {
      const rect = (await this.single("boundingBox")).rect;
      if (!rect) throw new Error(`boundingBox(): ${this.target()} is not visible`);
      return { x: rect.left, y: rect.top, width: rect.width, height: rect.height };
    });
  }

  touchTarget(): Promise<{ x: number; y: number; width: number; height: number }> {
    return this.read("touchTarget", async () => {
      const resolved = await this.single("touchTarget");
      const rect = resolved.targetRect ?? resolved.rect;
      if (!rect) throw new Error(`touchTarget(): ${this.target()} is not visible`);
      return { x: rect.left, y: rect.top, width: rect.width, height: rect.height };
    });
  }

  /** A traced one-shot read. */
  private read<T>(verb: string, op: () => Promise<T>): Promise<T> {
    return this.record(`page.${verb}`, this.target(), op);
  }

  private async single(verb: string, attributes: readonly string[] = []): Promise<LocatorResolve> {
    const resolved = await this.resolve(undefined, verb, attributes);
    if (resolved.count !== 1) {
      throw new Error(`${verb}(): ${this.missText(resolved)}` +
        (resolved.kind === "many" ? "; narrow it with nth(), first(), last() or filter()" : await this.onPage()));
    }
    return resolved;
  }

  /** For a read that found nothing: the test ids a GPU page shows instead, so a page that had not
   *  rendered yet, or another page, reads as such. A WebView page answers no attributes in a query. */
  private async onPage(): Promise<string> {
    try {
      const all = await this.page.query({ page: this.options.page, css: "[data-testid]", all: true });
      const items = Array.isArray(all.items) ? all.items : [];
      if (!items.some((item) => item.renderer === "gpu")) return "";
      const ids = items.filter((item) => item.visible && item.inViewport !== false)
        .map((item) => item.attributes?.["data-testid"])
        .filter((id): id is string => typeof id === "string");
      if (ids.length === 0) return "; no element with a test id is on screen";
      const more = ids.length > 8 ? ` and ${ids.length - 8} more` : "";
      return `; on screen: ${ids.slice(0, 8).join(", ")}${more}`;
    } catch {
      return "";
    }
  }

  /**
   * One read of the locator's matches. With a deadline the query is raced
   * against what is left of it, so a query that never returns fails `verb`.
   */
  async resolve(deadline?: ActionDeadline, verb = "resolve", attributes: readonly string[] = []): Promise<LocatorResolve> {
    const query = () => this.page.query({ page: this.options.page, css: this.selector, all: true })
      .catch((error: unknown) => { throw this.scopedRefusal(error); });
    const all = await this.guard(() =>
      deadline ? deadline.call("page.query", query, () => this.callContext(verb)) : query(),
    );
    const raw = Array.isArray(all.items) ? all.items : all.exists ? [all] : [];
    // Every match keeps its DOM index: the driver addresses `css` + index.
    // Text is read as the user sees it: `innerText` line breaks depend on
    // layout (WebKit ends a flex row's text in "\n"), so whitespace is
    // collapsed and trimmed before any matcher sees it.
    const indexed = raw.map((item, position) => ({
      ...item,
      index: item.index ?? position,
      ...(typeof item.text === "string" ? { text: normalizeText(item.text) } : {}),
    }));
    const hasText = this.refine.hasText;
    const name = this.refine.name;
    const selected = this.refine.selected;
    const matches = indexed
      .filter((item) => hasText === undefined || textMatches(item.text ?? "", hasText))
      .filter((item) => name === undefined || nameMatches(item, name))
      .filter((item) => selected === undefined || (item.selected ?? false) === selected);
    const pick = this.refine.last ? matches.length - 1 : this.options.index;
    const picked = pick === undefined ? undefined : matches[pick];
    const items: QueryMatch[] = pick === undefined ? matches : picked ? [picked] : [];
    const narrowed = hasText !== undefined || name !== undefined || selected !== undefined;
    const count = pick !== undefined ? items.length : narrowed ? items.length : (all.count ?? items.length);
    const visibleItems = items.filter((item) => item.visible);
    const visibleCount = visibleItems.length;
    let resolved: LocatorResolve;
    if (count === 0) {
      resolved = {
        count: 0,
        visibleCount: 0,
        attached: false,
        visible: false,
        inViewport: false,
        text: "",
        value: null,
        index: 0,
        kind: "nothing",
      };
    } else if (count > 1) {
      resolved = { count, visibleCount, attached: true, visible: visibleCount > 0, inViewport: false,
        text: items.map(item => item.text ?? "").join("\n"), value: null, index: 0, kind: "many" };
    } else if (visibleCount === 0) {
      resolved = {
        count,
        visibleCount: 0,
        attached: true,
        visible: false,
        inViewport: false,
        text: items[0]?.text ?? "",
        value: items[0]?.value ?? null,
        index: items[0]?.index ?? 0,
        enabled: items[0]?.enabled, editable: items[0]?.editable, focused: items[0]?.focused,
        ...states(items[0]),
        kind: "hidden",
      };
    } else {
      const unique = visibleItems[0]!;
      resolved = {
        count,
        visibleCount: 1,
        attached: true,
        visible: true,
        inViewport: unique.inViewport === true,
        text: unique.text ?? "",
        value: unique.value ?? null,
        index: unique.index ?? items.indexOf(unique),
        enabled: unique.enabled, editable: unique.editable, focused: unique.focused, rect: unique.rect,
        ...states(unique),
        ...(unique.visibleRect !== undefined ? { visibleRect: unique.visibleRect } : {}),
        ...(unique.targetRect ? { targetRect: unique.targetRect } : {}),
        ...(unique.renderer ? { renderer: unique.renderer } : {}),
        kind: "unique",
      };
    }
    if (attributes.length > 0 && count === 1) {
      const item = visibleItems[0] ?? items[0];
      // A GPU page answers attributes in its query; a WebView reads them from the DOM.
      resolved.attributes = item?.renderer === "gpu"
        ? Object.fromEntries(attributes.map((name) => [name, item.attributes?.[name] ?? null]))
        : await this.readAttributes(resolved.index, attributes, deadline, verb);
    }
    return resolved;
  }

  /** A read-only look at the single match's attributes. */
  private async readAttributes(
    index: number,
    names: readonly string[],
    deadline: ActionDeadline | undefined,
    verb: string,
  ): Promise<Record<string, string | null>> {
    if (!this.page.eval) throw new Error("This page driver cannot read attributes");
    const script = `(() => {
      const el = document.querySelectorAll(${JSON.stringify(this.selector)})[${index}];
      const out = {};
      for (const name of ${JSON.stringify(names)}) out[name] = el ? el.getAttribute(name) : null;
      return out;
    })()`;
    const read = () => this.page.eval!({ page: this.options.page, script });
    const value = await this.guard(() =>
      deadline ? deadline.call("page.eval (attributes)", read, () => this.callContext(verb)) : read());
    const out: Record<string, string | null> = {};
    const record = value && typeof value === "object" ? value as Record<string, unknown> : {};
    for (const name of names) out[name] = typeof record[name] === "string" ? record[name] as string : null;
    return out;
  }

  /** An old GPU host refusing a scoped query says so in words, keeping the driver code. */
  private scopedRefusal(error: unknown): unknown {
    const message = error instanceof Error ? error.message : String(error);
    if (!this.refine.scoped || errorCode(error) !== "E_SELECTOR_UNSUPPORTED" && !message.startsWith("E_SELECTOR_UNSUPPORTED")) return error;
    if (message.includes(SCOPED_GPU_QUERIES)) return error;
    return Object.assign(new Error(`this app's View predates scoped queries; rebuild the host to use Locator.getByRole (${formatValue(this.selector)})`), {
      code: "E_SELECTOR_UNSUPPORTED", cause: error,
    });
  }

  missText(resolved: LocatorResolve): string {
    if (resolved.kind === "unique" && !resolved.inViewport) {
      return `locator ${formatValue(this.selector)} resolved to a visible element outside the viewport`;
    }
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

  /** The page instance this locator is bound to; `null` for the current page. */
  get boundPage(): string | null {
    return this.options.page ?? null;
  }

  /** The page, selector and narrowing, as the trace and failures show it. */
  describe(): string {
    return this.target();
  }

  private inputDetail(text: string): string {
    // Locator names commonly reveal password fields. Declared --secret-arg
    // values are also scrubbed from the final report by the run redactor.
    if (/(?:password|passwd|passphrase|secret|token|credential|api[-_]?key)/i.test(this.selector)) {
      return `value=[redacted, ${text.length} chars]`;
    }
    return `value=${formatValue(text)}`;
  }

  private target(): string {
    const filter = (this.refine.hasText === undefined ? "" : ` filter(hasText=${formatValue(this.refine.hasText)})`)
      + (this.refine.name === undefined ? "" : ` name=${formatValue(this.refine.name)}`)
      + (this.refine.selected === undefined ? "" : ` selected=${this.refine.selected}`);
    const pick = this.refine.last ? " [last]" : this.options.index === undefined ? "" : ` [${this.options.index}]`;
    return `${this.options.page ? `#${this.options.page} ` : ""}${this.selector}${filter}${pick}`;
  }

  private where(): string {
    return displayLocation(this.location.source, this.location.line, this.location.column);
  }

  private callContext(verb: string): string {
    return `while trying to ${verb} ${formatValue(this.selector)}\nat ${this.where()}`;
  }

  /** @internal Scroll match `index` on screen through the page driver. */
  async reveal(index: number, verb: string, deadline: ActionDeadline): Promise<void> {
    if (!this.page.scrollTo) return;
    await this.guard(() => deadline.call("page.scrollTo", () =>
      this.page.scrollTo!({ page: this.options.page, css: this.selector, index }), () => this.callContext(verb)));
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

  private act(
    verb: string,
    options: ActionOptions | undefined,
    run: (css: string, index: number) => Promise<void>,
    input?: string,
  ): Promise<void> {
    const force = options?.force === true;
    const timeout = options?.timeout ?? DEFAULT_ACTION_TIMEOUT_MS;
    const interval = DEFAULT_POLL_INTERVAL_MS;
    if (!Number.isFinite(timeout) || timeout <= 0) {
      return Promise.reject(new TypeError("Action timeout must be a positive finite number"));
    }
    const deadline = new ActionDeadline(timeout, this.room());
    const context = () => this.callContext(verb);
    return this.record(`page.${verb}`, `${this.target()}${input ? ` ${input}` : ""}`, async () => {
      let last: LocatorResolve | undefined;
      let previousRect: string | undefined;
      let reason = "not attached";
      // Preserve the last driver failure as the timeout cause.
      let cause: unknown;
      // The native act's last refusal: the stability wait it restarts must not hide it.
      let refusal: { reason: string; cause: unknown } | undefined;
      const until = async (budget: ActionDeadline): Promise<boolean> => {
        while (!budget.expired()) {
          try {
            last = await this.resolve(budget, verb);
            reason = this.missText(last);
            cause = undefined;
            if (last.kind === "unique" && last.renderer === "gpu" && !last.inViewport) {
              // A GPU page acts through the presented frame, so the match must be on screen first.
              await this.reveal(last.index, verb, budget);
              reason = "element is outside the viewport";
              previousRect = undefined;
            } else if (force && last.count === 1) {
              // Forced: attached and enabled are enough; no viewport, stability
              // or hit-test wait.
              if (last.enabled === false) reason = "element is disabled";
              else if ((verb === "fill" || verb === "type") && last.editable === false) reason = "element is not editable";
              else {
                try {
                  const index = last.index;
                  await this.guard(() => budget.call(`page.${verb}`, () => run(this.selector, index), context));
                  return true;
                } catch (error) {
                  if (!isElementRefusal(error) && !isPreDispatchPageError(error)) {
                    throw new DispatchFailure(error);
                  }
                  reason = error instanceof Error ? error.message : String(error);
                  cause = error;
                }
              }
            } else if (last.kind === "unique") {
              const rect = JSON.stringify(last.rect);
              const stable = last.rect === undefined || previousRect === rect;
              previousRect = rect;
              if (last.enabled === false) reason = "element is disabled";
              else if ((verb === "fill" || verb === "type") && last.editable === false) reason = "element is not editable";
              else if (!stable) {
                reason = refusal?.reason ?? "element is moving";
                cause = refusal?.cause;
              }
              else {
                // A GPU page has no DOM to probe; the native act refuses what it cannot take.
                const check = last.renderer === "gpu" ? true : await this.actionability(last.index, verb, budget);
                if (check === true) {
                  try {
                    const index = last.index;
                    await this.guard(() => budget.call(`page.${verb}`, () => run(this.selector, index), context));
                    return true;
                  } catch (error) {
                    // These native errors are raised before input dispatch. A transport
                    // failure is ambiguous and must never resubmit an action.
                    if (!isElementRefusal(error) && !isPreDispatchPageError(error)) {
                      throw new DispatchFailure(error);
                    }
                    reason = error instanceof Error ? error.message : String(error);
                    cause = error;
                    refusal = { reason, cause };
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
            // Reads only: a dispatch error is a DispatchFailure, never retried.
            if (!isTransientPageError(error) && !isTransientTransportError(error)) throw error;
            reason = transientReason(error);
            cause = error;
            previousRect = undefined;
          }
          await sleep(Math.min(interval, Math.max(1, budget.remaining())));
        }
        return false;
      };
      if (await until(deadline)) return;
      throw withCause(new TimeoutError([
        `Timed out after ${deadline.elapsed()}ms waiting to ${verb} ${formatValue(this.selector)}.`,
        reason,
        deadline.clampNote(),
        `at ${this.where()}`,
      ].filter(Boolean).join("\n")), cause);
    });
  }
}

/** Preserve the last driver failure without changing the public timeout code. */
function withCause(error: TimeoutError, cause: unknown): TimeoutError {
  if (cause === undefined) return error;
  Object.assign(error, { cause });
  const code = errorCode(cause);
  const data = cause && typeof cause === "object" ? (cause as { data?: unknown }).data : undefined;
  error.data = {
    ...(data && typeof data === "object" ? data : {}),
    ...(code === undefined ? {} : { driverCode: code }),
  };
  return error;
}

/** A dispatch error that must propagate as is, never be retried. */
class DispatchFailure {
  constructor(readonly error: unknown) {}
}

function transientReason(error: unknown): string {
  const text = error instanceof Error ? error.message : String(error);
  return isTransientTransportError(error) ? `read dropped by the transport: ${text}` : `page not ready: ${text}`;
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
    case "inViewport":
      return resolved.kind === "unique" && resolved.inViewport;
  }
}

/** How long the visibility probe waits for an animation frame. */
const VISIBILITY_FRAME_WAIT_MS = 300;
/** The probe's whole budget: a hidden page also throttles its timers. */
export const VISIBILITY_PROBE_BUDGET_MS = 1_500;

/**
 * Reads `document.visibilityState` and whether an animation frame arrives.
 * WebKit stops animation frames for a page that is hidden or in a fully
 * covered window, so rAF- and transition-driven UI (a sheet sliding in)
 * never moves and actionability reads it as obscured.
 */
export const VISIBILITY_PROBE_SCRIPT = `new Promise((resolve) => {
  let done = false;
  const finish = (frame) => {
    if (done) return;
    done = true;
    resolve({ state: String(document.visibilityState || "unknown"), animationFrames: frame });
  };
  try { requestAnimationFrame(() => finish(true)); } catch (_) { finish(false); }
  setTimeout(() => finish(false), ${VISIBILITY_FRAME_WAIT_MS});
})`;

/**
 * What the probe can say: what it observed, and what usually causes that. It
 * cannot see the window, so the cause stays a likely one.
 */
export const HIDDEN_PAGE_NOTE = "page looked hidden or paused";
const HIDDEN_PAGE_CAUSE = "a covered window or a sleeping display pauses a page's animations";

/** Why every page is hidden when the host can tell the screen is locked. */
export const SCREEN_LOCKED_NOTE = "the screen is locked; unlock it — animations and sheets are paused";

/** A hidden-page note, naming the lock when that is the cause. */
export function hiddenCause(note: string | undefined): string | undefined {
  return note !== undefined && resolveHost().screenLocked() ? SCREEN_LOCKED_NOTE : note;
}

/** What the probe saw, or `undefined` for an unreadable answer. */
export function pageVisibility(value: unknown): PageVisibility | undefined {
  if (!value || typeof value !== "object") return undefined;
  const record = value as { state?: unknown; animationFrames?: unknown };
  if (typeof record.state !== "string" || typeof record.animationFrames !== "boolean") return undefined;
  const hidden = record.state === "hidden" || !record.animationFrames;
  return {
    state: record.state,
    animationFrames: record.animationFrames,
    ...(hidden ? { note: `${HIDDEN_PAGE_NOTE}: visibilityState "${record.state}", ${record.animationFrames ? "animation frames running" : `no animation frame observed within ${VISIBILITY_FRAME_WAIT_MS}ms`} (${HIDDEN_PAGE_CAUSE})` } : {}),
  };
}

/** Collapse every run of whitespace (newlines included) to one space and trim. */
export function normalizeText(value: string): string {
  return value.replace(/\s+/g, " ").trim();
}

/** The accessible name: `aria-label` when set, else the visible text; a string matches whole. */
function nameMatches(item: QueryMatch, expected: string | RegExp): boolean {
  const label = typeof item.ariaLabel === "string" && item.ariaLabel.trim() ? normalizeText(item.ariaLabel) : item.text ?? "";
  return typeof expected === "string" ? label === normalizeText(expected) : expected.test(label);
}

export function textMatches(text: string, expected: string | RegExp): boolean {
  if (expected instanceof RegExp) {
    expected.lastIndex = 0;
    return expected.test(text);
  }
  return normalizeText(text).toLowerCase().includes(normalizeText(expected).toLowerCase());
}

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    runnerSetTimeout(resolve, ms);
  });
}
