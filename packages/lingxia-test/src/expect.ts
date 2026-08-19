import { formatValue } from "./format.js";
import type { Matchers } from "./types.js";

export interface LoggedAssertion {
  matcher: string;
  expected: string;
  actual: string;
  passed: boolean;
}

type AssertionSink = (entry: LoggedAssertion) => void;

let assertionSink: AssertionSink | undefined;
let assertionSilence = 0;

export function setAssertionSink(sink?: AssertionSink): void {
  assertionSink = sink;
}

export function pushAssertionSilence(): void {
  assertionSilence += 1;
}

export function popAssertionSilence(): void {
  assertionSilence = Math.max(0, assertionSilence - 1);
}

export function logAssertion(entry: LoggedAssertion): void {
  if (assertionSilence > 0 || !assertionSink) return;
  assertionSink(entry);
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

function isEqual(a: unknown, b: unknown, seen = new WeakMap<object, unknown>()): boolean {
  if (Object.is(a, b)) return true;
  if (typeof a !== typeof b) return false;
  if (a === null || b === null) return a === b;
  if (typeof a !== "object" || typeof b !== "object") return false;
  if (a instanceof Date || b instanceof Date) {
    return a instanceof Date && b instanceof Date && a.getTime() === b.getTime();
  }
  if (a instanceof RegExp || b instanceof RegExp) {
    return a instanceof RegExp && b instanceof RegExp && String(a) === String(b);
  }
  if (a instanceof Map || b instanceof Map) {
    if (!(a instanceof Map) || !(b instanceof Map) || a.size !== b.size) return false;
    for (const [key, value] of a) {
      if (!b.has(key) || !isEqual(value, b.get(key), seen)) return false;
    }
    return true;
  }
  if (a instanceof Set || b instanceof Set) {
    if (!(a instanceof Set) || !(b instanceof Set) || a.size !== b.size) return false;
    for (const value of a) {
      let found = false;
      for (const other of b) {
        if (isEqual(value, other, seen)) {
          found = true;
          break;
        }
      }
      if (!found) return false;
    }
    return true;
  }
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  if (seen.get(a) === b) return true;
  seen.set(a, b);
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    return a.every((item, index) => isEqual(item, b[index], seen));
  }
  const aKeys = Object.keys(a as object);
  const bKeys = Object.keys(b as object);
  if (aKeys.length !== bKeys.length) return false;
  return aKeys.every((key) =>
    Object.prototype.hasOwnProperty.call(b, key) &&
    isEqual((a as Record<string, unknown>)[key], (b as Record<string, unknown>)[key], seen)
  );
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
  logAssertion({
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

function createMatchers<T>(actual: T, inverted: boolean): Matchers<T> {
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
  return self;
}

export function expect<T>(actual: T): Matchers<T> {
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
    default:
      throw new AssertionError(matcher, actual, expected, `Unknown matcher ${matcher}`);
  }
}

export { isEqual, contains, matches };
