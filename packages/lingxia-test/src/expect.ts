import { isEqual } from "./equal.js";
import { formatValue } from "./format.js";
import { activeOpenApi, type SchemaTarget } from "./openapi.js";
import { formatIssues } from "./schema.js";
import { displayLocation, parseFrames, resolveOrigin } from "./ids.js";
import type { Expect, ExpectOptions, FunctionMatchers, Locator, LocatorMatchers, Matchers, RetryMatchers } from "./types.js";

export interface LoggedAssertion {
  matcher: string;
  expected: string;
  actual: string;
  passed: boolean;
}

/**
 * The running spec, as `expect` needs it: to record assertions, and to run
 * the retrying forms (`expect(locator)`, `expect.poll`) inside its budget.
 */
export interface ExpectScope {
  note(entry: LoggedAssertion): void;
  locator(locator: Locator): LocatorMatchers;
  poll(read: () => unknown, options: ExpectOptions | undefined): RetryMatchers<unknown>;
}

let activeScope: ExpectScope | undefined;
let assertionSilence = 0;
/** Where an assertion goes that ran while no spec was running. */
let straySink: ((entry: LoggedAssertion) => void) | undefined;

/** The spec `expect` reports to; `undefined` between specs. */
export function setExpectScope(scope?: ExpectScope): void {
  activeScope = scope;
}

/**
 * During a run, assertions made while no spec runs — a continuation of a
 * spec the run already finished — are handed here instead of to whichever
 * spec runs next.
 */
export function setStrayAssertionSink(sink?: (entry: LoggedAssertion) => void): void {
  straySink = sink;
}

export function pushAssertionSilence(): void {
  assertionSilence += 1;
}

export function popAssertionSilence(): void {
  assertionSilence = Math.max(0, assertionSilence - 1);
}

function recordAssertion(entry: LoggedAssertion): void {
  if (assertionSilence > 0) return;
  if (activeScope) activeScope.note(entry);
  else straySink?.(entry);
}

export class AssertionError extends Error {
  override readonly name = "AssertionError";
  readonly matcher: string;
  readonly actual: unknown;
  readonly expected: unknown;

  constructor(matcher: string, actual: unknown, expected: unknown, message: string) {
    super(message);
    this.matcher = matcher;
    this.actual = actual;
    this.expected = expected;
  }
}


function contains(actual: unknown, expected: unknown): boolean {
  if (typeof actual === "string") return actual.includes(String(expected));
  if (Array.isArray(actual)) return actual.some((item) => isEqual(item, expected));
  return false;
}

function matches(actual: unknown, expected: string | RegExp): boolean {
  const text = typeof actual === "string" ? actual : formatValue(actual);
  return typeof expected === "string" ? text.includes(expected) : expected.test(text);
}

function message(
  matcher: string,
  actual: unknown,
  expected: unknown,
  inverted: boolean,
  extra?: string,
): string {
  const lines = [
    inverted ? `expect(received).not.${matcher}` : `expect(received).${matcher}`,
    extra ?? "",
    `Expected: ${inverted ? "not " : ""}${formatValue(expected)}`,
    `Received: ${formatValue(actual)}`,
  ].filter((line) => line.length > 0);
  return lines.join("\n");
}

function settle(
  matcher: string,
  actual: unknown,
  expected: unknown,
  inverted: boolean,
  pass: boolean,
  extra?: string,
): void {
  const ok = pass !== inverted;
  recordAssertion({
    matcher: inverted ? `not.${matcher}` : matcher,
    expected: inverted ? `not ${formatValue(expected)}` : formatValue(expected),
    actual: formatValue(actual),
    passed: ok,
  });
  if (!ok) fail(matcher, actual, expected, inverted, extra);
}

function fail(
  matcher: string,
  actual: unknown,
  expected: unknown,
  inverted: boolean,
  extra?: string,
): never {
  throw new AssertionError(
    inverted ? `not.${matcher}` : matcher,
    actual,
    expected,
    message(matcher, actual, expected, inverted, extra),
  );
}

function isComparable(value: unknown): value is number {
  return typeof value === "number" && !Number.isNaN(value);
}

function createMatchers<T>(actual: T, inverted: boolean): Matchers<T> {
  const compare = (
    matcher: string,
    expected: number,
    pass: (actual: number, expected: number) => boolean,
  ) => {
    if (!isComparable(actual) || !isComparable(expected)) {
      // Comparing a non-number is a broken assertion, not a threshold that was
      // missed, so it fails even under `.not` -- otherwise a misspelled field
      // reads as "correctly not greater" and the spec passes on nothing.
      // Logged under the matcher the spec actually wrote, `.not` included.
      recordAssertion({
        matcher: inverted ? `not.${matcher}` : matcher,
        expected: formatValue(expected),
        actual: formatValue(actual),
        passed: false,
      });
      fail(matcher, actual, expected, inverted, "both values must be numbers");
    }
    settle(matcher, actual, expected, inverted, pass(actual, expected));
  };

  const self = {
    get not(): Matchers<T> {
      return createMatchers(actual, !inverted);
    },
    toBe(expected: unknown) {
      settle("toBe", actual, expected, inverted, Object.is(actual, expected));
    },
    toEqual(expected: unknown) {
      settle("toEqual", actual, expected, inverted, isEqual(actual, expected));
    },
    toContain(expected: unknown) {
      settle("toContain", actual, expected, inverted, contains(actual, expected));
    },
    toMatch(expected: string | RegExp) {
      settle("toMatch", actual, expected, inverted, matches(actual, expected));
    },
    toBeTruthy() {
      settle("toBeTruthy", actual, true, inverted, Boolean(actual));
    },
    toBeFalsy() {
      settle("toBeFalsy", actual, false, inverted, !actual);
    },
    toBeDefined() {
      settle("toBeDefined", actual, undefined, inverted, actual !== undefined);
    },
    toBeUndefined() {
      settle("toBeUndefined", actual, undefined, inverted, actual === undefined);
    },
    toBeInstanceOf(expected: Function) {
      const pass = actual instanceof (expected as new (...args: never[]) => unknown);
      settle("toBeInstanceOf", actual, expected, inverted, pass);
    },
    toHaveLength(expected: number) {
      const length = (actual as { length?: unknown } | null | undefined)?.length;
      if (typeof length !== "number") {
        recordAssertion({ matcher: inverted ? "not.toHaveLength" : "toHaveLength", expected: formatValue(expected), actual: formatValue(actual), passed: false });
        fail("toHaveLength", actual, expected, inverted, "received value must have a numeric length");
      }
      settle("toHaveLength", length, expected, inverted, length === expected);
    },
    toBeGreaterThan(expected: number) {
      compare("toBeGreaterThan", expected, (a, b) => a > b);
    },
    toBeGreaterThanOrEqual(expected: number) {
      compare("toBeGreaterThanOrEqual", expected, (a, b) => a >= b);
    },
    toBeLessThan(expected: number) {
      compare("toBeLessThan", expected, (a, b) => a < b);
    },
    toBeLessThanOrEqual(expected: number) {
      compare("toBeLessThanOrEqual", expected, (a, b) => a <= b);
    },
    toMatchSchema(schema: SchemaTarget) {
      const index = activeOpenApi();
      let result: ReturnType<NonNullable<typeof index>["validate"]> | undefined;
      let broken: string | undefined;
      if (!index) {
        broken = "toMatchSchema needs an OpenAPI document: run `lxdev test --openapi <spec.yaml|json>`";
      } else {
        try {
          result = index.validate(actual, schema);
        } catch (error) {
          broken = error instanceof Error ? error.message : String(error);
        }
      }
      if (!result) {
        // A schema that cannot be found is a broken assertion, not a
        // mismatch: it fails under `.not` too.
        recordAssertion({
          matcher: inverted ? "not.toMatchSchema" : "toMatchSchema",
          expected: formatValue(schema),
          actual: formatValue(actual),
          passed: false,
        });
        throw new AssertionError(inverted ? "not.toMatchSchema" : "toMatchSchema", actual, schema, broken ?? "toMatchSchema failed");
      }
      const label = `${result.pointer} in ${result.document}`;
      const detail = result.issues.length > 0 ? `Schema mismatches:\n${formatIssues(result.issues)}` : undefined;
      settle("toMatchSchema", actual, label, inverted, result.issues.length === 0, detail);
    },
    toThrow(expected?: unknown) {
      if (typeof actual !== "function") {
        settle("toThrow", actual, expected, inverted, false, "received value must be a function");
        return;
      }
      let thrown: unknown;
      let didThrow = false;
      try {
        (actual as () => unknown)();
      } catch (error) {
        didThrow = true;
        thrown = error;
      }
      let pass = didThrow;
      if (pass && expected !== undefined) {
        const text = thrown instanceof Error ? thrown.message : String(thrown);
        if (typeof expected === "string") pass = text.includes(expected);
        else if (expected instanceof RegExp) pass = expected.test(text);
        else if (typeof expected === "function") pass = thrown instanceof expected;
        else pass = isEqual(thrown, expected);
      }
      settle("toThrow", thrown, expected, inverted, pass);
    },
  };
  // `then` is added where `expect` hands the matchers out (`refuseAwait`).
  return self as unknown as Matchers<T>;
}

/** `LOCATOR_BRAND` in `locator.ts`; read by key to keep this module free of it. */
const LOCATOR_BRAND = Symbol.for("lingxia.test.locator");

function isLocator(value: unknown): value is Locator {
  return typeof value === "object" && value !== null && (value as { [LOCATOR_BRAND]?: unknown })[LOCATOR_BRAND] === true;
}

function isThenable(value: unknown): boolean {
  return (typeof value === "object" || typeof value === "function") && value !== null &&
    typeof (value as { then?: unknown }).then === "function";
}

function runningScope(api: string): ExpectScope {
  if (!activeScope) throw new Error(`${api} retries inside a spec's budget; call it from a running spec`);
  return activeScope;
}

/**
 * `expect(fn)`: `toThrow` calls it once. Any other matcher would compare the
 * function itself, which is never what a spec means: a read to retry is
 * `expect.poll(read)`.
 */
function functionMatchers(fn: unknown, inverted: boolean): FunctionMatchers {
  const matchers = createMatchers(fn, inverted);
  const out: Record<string, unknown> = {};
  for (const key of Object.keys(matchers)) {
    if (key === "not") continue;
    out[key] = key === "toThrow"
      ? matchers.toThrow
      : () => {
        throw new TypeError(`expect(fn).${key}: a function is only called by toThrow; to retry a read until it passes, use expect.poll(read)`);
      };
  }
  Object.defineProperty(out, "not", { get: () => functionMatchers(fn, !inverted), enumerable: true, configurable: true });
  return out as unknown as FunctionMatchers;
}

/**
 * Awaiting the matchers object itself checks nothing. Make it reject, naming
 * where `expect` was called, instead of passing silently.
 */
function refuseAwait<M extends object>(matchers: M, api: string, example: string, origin: Error): M {
  const not = Object.getOwnPropertyDescriptor(matchers, "not");
  if (not?.get) {
    const get = not.get;
    Object.defineProperty(matchers, "not", {
      get: () => refuseAwait(get.call(matchers) as object, api, example, origin),
      enumerable: not.enumerable,
      configurable: true,
    });
  }
  Object.defineProperty(matchers, "then", {
    value: (_resolve: unknown, reject?: (reason: unknown) => void) => {
      const at = resolveOrigin(parseFrames(origin.stack));
      const error = new TypeError(
        `${api} checks nothing until a matcher is called: ${example}\nat ${displayLocation(at.file, at.line, at.column)}`,
      );
      if (typeof reject === "function") reject(error);
      else throw error;
    },
    enumerable: false,
    configurable: true,
  });
  return matchers;
}

/**
 * The one assertion entry point: a locator retries its matcher until the
 * element passes, any other value is checked once, and `expect.poll(read)`
 * calls `read` until the matcher passes.
 */
export const expect: Expect = Object.assign(
  (subject: unknown) => {
    const origin = new Error();
    if (isLocator(subject)) {
      return refuseAwait(runningScope("expect(locator)").locator(subject), "expect(locator)",
        "await expect(locator).toBeVisible()", origin);
    }
    if (isThenable(subject)) {
      throw new TypeError("expect(promise): await the value first, or retry a read with expect.poll(() => promise)");
    }
    if (typeof subject === "function") {
      return refuseAwait(functionMatchers(subject, false), "expect(fn)", "expect(fn).toThrow()", origin);
    }
    return refuseAwait(createMatchers(subject, false), "expect(value)", "expect(value).toBe(expected)", origin);
  },
  {
    poll: (read: () => unknown, options?: ExpectOptions) => {
      const origin = new Error();
      if (typeof read !== "function") throw new TypeError("expect.poll(read) takes a function to call until the matcher passes");
      return refuseAwait(runningScope("expect.poll(read)").poll(read, options), "expect.poll(read)",
        "await expect.poll(read).toBe(expected)", origin);
    },
  },
) as Expect;

/** Check `actual` once; the fixture's own checks use it. */
export function check<T>(actual: T): Matchers<T> {
  return createMatchers(actual, false);
}

export function applyMatcher(
  matcher: string,
  actual: unknown,
  expected: unknown,
  inverted: boolean,
): void {
  const assertion = createMatchers(actual, inverted);
  switch (matcher) {
    case "toBe":
      assertion.toBe(expected);
      return;
    case "toEqual":
      assertion.toEqual(expected);
      return;
    case "toContain":
      assertion.toContain(expected);
      return;
    case "toMatch":
      assertion.toMatch(expected as string | RegExp);
      return;
    case "toBeTruthy":
      assertion.toBeTruthy();
      return;
    case "toBeFalsy":
      assertion.toBeFalsy();
      return;
    case "toBeDefined":
      assertion.toBeDefined();
      return;
    case "toBeUndefined":
      assertion.toBeUndefined();
      return;
    case "toBeInstanceOf":
      assertion.toBeInstanceOf(expected as Function);
      return;
    case "toHaveLength":
      assertion.toHaveLength(expected as number);
      return;
    case "toBeGreaterThan":
      assertion.toBeGreaterThan(expected as number);
      return;
    case "toBeGreaterThanOrEqual":
      assertion.toBeGreaterThanOrEqual(expected as number);
      return;
    case "toBeLessThan":
      assertion.toBeLessThan(expected as number);
      return;
    case "toBeLessThanOrEqual":
      assertion.toBeLessThanOrEqual(expected as number);
      return;
    default:
      throw new AssertionError(matcher, actual, expected, `Unknown matcher ${matcher}`);
  }
}

export { isEqual, contains, matches };
