import assert from "node:assert/strict";
import { test } from "node:test";
import { expect } from "../dist/expect.js";

function failure(body) {
  try {
    body();
  } catch (error) {
    return error;
  }
  return undefined;
}

test("orders numbers on both sides of each boundary", () => {
  expect(2).toBeGreaterThan(1);
  expect(2).toBeGreaterThanOrEqual(2);
  expect(1).toBeLessThan(2);
  expect(2).toBeLessThanOrEqual(2);

  assert.equal(failure(() => expect(1).toBeGreaterThan(1))?.matcher, "toBeGreaterThan");
  assert.equal(failure(() => expect(1).toBeGreaterThanOrEqual(2))?.matcher, "toBeGreaterThanOrEqual");
  assert.equal(failure(() => expect(2).toBeLessThan(2))?.matcher, "toBeLessThan");
  assert.equal(failure(() => expect(3).toBeLessThanOrEqual(2))?.matcher, "toBeLessThanOrEqual");
});

test("reports both numbers, which is the whole reason to have these", () => {
  const error = failure(() => expect(1_499_000).toBeGreaterThan(1_500_000));
  // `expect(a > b).toBeTruthy()` could only ever report "expected true", so a
  // failure read back from a report cost a re-run to learn the actual value.
  assert.match(error.message, /1499000/);
  assert.match(error.message, /1500000/);
});

test("inverts", () => {
  expect(1).not.toBeGreaterThan(2);
  expect(2).not.toBeLessThan(1);
  assert.equal(failure(() => expect(2).not.toBeGreaterThan(1))?.matcher, "not.toBeGreaterThan");
});

test("treats a non-number as a broken assertion, even when inverted", () => {
  // A misspelled field must not read as "correctly not greater" and pass.
  for (const body of [
    () => expect(undefined).toBeGreaterThan(0),
    () => expect("2").toBeGreaterThan(1),
    () => expect(Number.NaN).toBeLessThan(1),
    () => expect(1).toBeGreaterThan(Number.NaN),
    () => expect(undefined).not.toBeGreaterThan(0),
    () => expect(1).not.toBeGreaterThan(undefined),
  ]) {
    assert.match(failure(body)?.message ?? "", /must be numbers/);
  }

  // The report has to name the assertion the spec wrote, not its positive twin.
  assert.equal(
    failure(() => expect(undefined).not.toBeGreaterThan(0))?.matcher,
    "not.toBeGreaterThan",
  );
});

test("compares infinities like the operators do", () => {
  expect(Number.POSITIVE_INFINITY).toBeGreaterThan(Number.MAX_SAFE_INTEGER);
  expect(Number.NEGATIVE_INFINITY).toBeLessThan(0);
});
