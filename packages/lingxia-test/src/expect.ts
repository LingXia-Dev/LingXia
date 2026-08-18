import { formatValue } from "./format.js";
import type { Matchers } from "./types.js";

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
      const pass = Object.is(actual, expected);
      if (pass === inverted) fail("toBe", actual, expected, inverted);
    },
    toEqual(expected: unknown) {
      const pass = isEqual(actual, expected);
      if (pass === inverted) fail("toEqual", actual, expected, inverted);
    },
    toContain(expected: unknown) {
      const pass = contains(actual, expected);
      if (pass === inverted) fail("toContain", actual, expected, inverted);
    },
    toMatch(expected: string | RegExp) {
      const pass = matches(actual, expected);
      if (pass === inverted) fail("toMatch", actual, expected, inverted);
    },
    toBeTruthy() {
      const pass = Boolean(actual);
      if (pass === inverted) fail("toBeTruthy", actual, true, inverted);
    },
    toBeFalsy() {
      const pass = !actual;
      if (pass === inverted) fail("toBeFalsy", actual, false, inverted);
    },
    toBeDefined() {
      const pass = actual !== undefined;
      if (pass === inverted) fail("toBeDefined", actual, undefined, inverted);
    },
    toBeUndefined() {
      const pass = actual === undefined;
      if (pass === inverted) fail("toBeUndefined", actual, undefined, inverted);
    },
    toBeInstanceOf(expected: Function) {
      const pass = actual instanceof (expected as new (...args: never[]) => unknown);
      if (pass === inverted) fail("toBeInstanceOf", actual, expected, inverted);
    },
    toThrow(expected?: unknown) {
      if (typeof actual !== "function") {
        fail("toThrow", actual, expected, inverted, "received value must be a function");
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
      if (pass === inverted) fail("toThrow", thrown, expected, inverted);
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
