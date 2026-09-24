import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, reset, run } from "../dist/index.js";
import { runBudget, shuffleWithSeed } from "../dist/runtime.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

const AUTO = { budgetPerSpecMs: "30000", budgetMinMs: "300000", budgetMaxMs: "3600000" };

test("the default budget scales with the planned executions", () => {
  assert.deepEqual(runBudget(AUTO, 4), { ms: 300_000, auto: true });
  assert.deepEqual(runBudget(AUTO, 40), { ms: 1_200_000, auto: true });
  assert.deepEqual(runBudget(AUTO, 400), { ms: 3_600_000, auto: true });
  assert.deepEqual(runBudget({ ...AUTO, budgetMs: "90000" }, 400), { ms: 90_000, auto: false });
  assert.equal(runBudget({}, 10), undefined);
});

test("a seeded shuffle is a reproducible permutation", () => {
  const items = Array.from({ length: 12 }, (_, i) => i);
  const once = shuffleWithSeed(items, 42);
  assert.deepEqual(shuffleWithSeed(items, 42), once);
  assert.deepEqual([...once].sort((a, b) => a - b), items);
  const orders = new Set(Array.from({ length: 10 }, (_, seed) => shuffleWithSeed(items, seed).join(",")));
  assert.ok(orders.size > 1, "different seeds give different orders");
});

test("shuffle and repeat-each plan every execution, reproducibly", async () => {
  const order = async (control) => {
    reset();
    const world = createWorld();
    const { events } = installFakeHost(world, { control });
    const ran = [];
    for (const name of ["alpha", "beta", "gamma", "delta"]) spec(name, async () => { ran.push(name); });
    const report = await run();
    return { ran, report, events };
  };
  const first = await order({ shuffle: "7", repeatEach: "2" });
  const second = await order({ shuffle: "7", repeatEach: "2" });
  assert.deepEqual(first.ran, second.ran);
  assert.equal(first.ran.length, 8);
  for (const name of ["alpha", "beta", "gamma", "delta"]) assert.equal(first.ran.filter((n) => n === name).length, 2);
  assert.equal(first.report.total, 8);
  assert.equal(first.report.passed, 8);
  assert.equal(first.report.meta.shuffle_seed, 7);
  assert.equal(first.report.meta.repeat_each, 2);
  const alpha = first.report.cases.filter((c) => c.id === "alpha");
  assert.deepEqual(alpha.map((c) => c.repeat).sort(), [1, 2]);
  assert.ok(alpha.every((c) => /\[repeat [12]\/2\]$/.test(c.full_name)), JSON.stringify(alpha.map((c) => c.full_name)));
  const started = first.events.find((e) => e.type === "run_started");
  assert.equal(started.total, 8);

  const plain = await order({ repeatEach: "3" });
  assert.deepEqual(plain.ran, ["alpha", "alpha", "alpha", "beta", "beta", "beta", "gamma", "gamma", "gamma", "delta", "delta", "delta"]);
});

test("an exhausted budget reports N/M and skips the rest as not run", async () => {
  const world = createWorld();
  const { events } = installFakeHost(world, { control: { budgetMs: "60" } });
  const ran = [];
  spec("slow", async () => { ran.push("slow"); await new Promise((resolve) => setTimeout(resolve, 90)); });
  spec("second", async () => { ran.push("second"); });
  spec("third", async () => { ran.push("third"); });

  const report = await run();
  assert.deepEqual(ran, ["slow"]);
  assert.equal(report.partial, true);
  assert.equal(report.skipped, 2);
  assert.deepEqual(report.meta.budget, { ms: 60, auto: false, planned: 3, exhausted_after: 1 });
  assert.match(report.cases[1].reason, /exhausted after 1\/3 specs/);
  const diagnostic = events.find((e) => e.type === "diagnostic" && e.phase === "budget");
  assert.match(diagnostic?.message ?? "", /exhausted after 1\/3 specs/);
});
