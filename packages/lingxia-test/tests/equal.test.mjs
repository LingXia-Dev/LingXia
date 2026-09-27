import assert from "node:assert/strict";
import { test } from "node:test";
import { isEqual } from "../dist/equal.js";

test("a Set element answers for one element on the other side", () => {
  assert.equal(isEqual(new Set([{ a: 1 }, { a: 1 }]), new Set([{ a: 1 }, { a: 2 }])), false);
  assert.equal(isEqual(new Set([{ a: 1 }, { a: 2 }]), new Set([{ a: 1 }, { a: 1 }])), false);
  assert.equal(isEqual(new Set([{ a: 1 }, { a: 1 }]), new Set([{ a: 1 }, { a: 1 }])), true);
  assert.equal(isEqual(new Set([1, { a: 1 }]), new Set([{ a: 1 }, 1])), true);
  assert.equal(isEqual(new Set([1, { a: 1 }]), new Set([{ a: 1 }, 2])), false);
});

test("Set contents match whatever their insertion order", () => {
  const left = new Set([{ tag: "x" }, { tag: "y" }, new Set([1])]);
  const right = new Set([new Set([1]), { tag: "y" }, { tag: "x" }]);
  assert.equal(isEqual(left, right), true);
  assert.equal(isEqual(left, new Set([new Set([2]), { tag: "y" }, { tag: "x" }])), false);
});

test("self-referential and mutually cyclic Sets compare without throwing", () => {
  const a = new Set(); a.add(a);
  const b = new Set(); b.add(b);
  assert.equal(isEqual(a, b), true);

  const a1 = new Set(); const a2 = new Set([a1]); a1.add(a2);
  const b1 = new Set(); const b2 = new Set([b1]); b1.add(b2);
  assert.equal(isEqual(a1, b1), true);

  const c = new Set([1]); c.add(c);
  const d = new Set([2]); d.add(d);
  assert.equal(isEqual(c, d), false);

  const loop = { items: new Set() }; loop.items.add(loop);
  const other = { items: new Set() }; other.items.add(other);
  assert.equal(isEqual(loop, other), true);
});

test("Maps with object keys match entries one to one", () => {
  const key = { id: 1 };
  assert.equal(isEqual(new Map([[key, "a"]]), new Map([[{ id: 1 }, "a"]])), true);
  assert.equal(isEqual(new Map([[{ id: 1 }, "a"]]), new Map([[{ id: 1 }, "b"]])), false);
  assert.equal(
    isEqual(new Map([[{ id: 1 }, "a"], [{ id: 1 }, "a"]]), new Map([[{ id: 1 }, "a"], [{ id: 2 }, "a"]])),
    false,
  );
  assert.equal(isEqual(new Map([["k", 1], [{ id: 1 }, 2]]), new Map([[{ id: 1 }, 2], ["k", 1]])), true);
  assert.equal(isEqual(new Map([["k", 1]]), new Map([["j", 1]])), false);

  const m = new Map(); m.set(m, m);
  const n = new Map(); n.set(n, n);
  assert.equal(isEqual(m, n), true);
});

test("a withdrawn assumption leaves no false pair for a later comparison", () => {
  // Inside the first element's failed match, `inner` is compared to
  // `innerOther` while the outer pair is assumed; that conclusion must not
  // survive into the second comparison.
  const x = { v: 1, self: null }; x.self = x;
  const y = { v: 2, self: null }; y.self = y;
  assert.equal(isEqual([x, x], [y, y]), false);
  assert.equal(isEqual(new Set([x]), new Set([y])), false);
});

test("expect.objectContaining matches the keys it names, at any depth, and says what it is", async () => {
  const { expect, AssertionError } = await import("../dist/index.js");
  const toast = { title: "Saved", icon: "success", duration: 1500, at: 1 };
  assert.equal(isEqual(toast, expect.objectContaining({ title: "Saved" })), true);
  assert.equal(isEqual(expect.objectContaining({ title: "Saved" }), toast), true);
  assert.equal(isEqual(toast, expect.objectContaining({ title: "Other" })), false);
  assert.equal(isEqual(toast, expect.objectContaining({ missing: undefined })), false, "a named key must exist");
  assert.equal(isEqual([toast], [expect.objectContaining({ icon: "success" })]), true);
  assert.equal(isEqual({ answer: { confirm: true, extra: 1 } }, { answer: expect.objectContaining({ confirm: true }) }), true);
  assert.equal(isEqual([1], expect.objectContaining({})), false, "an array is not an object sample");
  assert.throws(() => expect.objectContaining(null), /takes a plain object/);

  expect([toast]).toContainEqual(expect.objectContaining({ title: "Saved" }));
  expect([toast]).not.toContainEqual(expect.objectContaining({ title: "Other" }));
  assert.throws(() => expect([toast]).toContainEqual(expect.objectContaining({ title: "Other" })),
    (error) => error instanceof AssertionError && /Expected: ObjectContaining \{"title": "Other"\}/.test(error.message));
  assert.throws(() => expect("Saved").not.toContainEqual("x"), /received value must be an array/);
});
