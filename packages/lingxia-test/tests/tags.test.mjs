import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { registerOtherFileSpec } from "./helpers/other-file.mjs";
import { spec, reset, run, renderJUnit } from "../dist/index.js";
import { matchesTags, parseTagFilter, tagSummary, validateTags } from "../dist/tags.js";
import { coverageSummary, parseManifest } from "../dist/coverage.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

test("a --tag value is any-of its terms; several must all hold", () => {
  const has = (tags, ...values) => matchesTags(tags, parseTagFilter(values));
  assert.equal(has(["routed"], "routed"), true);
  assert.equal(has(["unit"], "routed"), false);
  assert.equal(has(["unit"], "routed,unit"), true);
  assert.equal(has(["live"], "!live"), false);
  assert.equal(has(["routed"], "!live"), true);
  assert.equal(has(["routed", "smoke"], "routed", "smoke"), true);
  assert.equal(has(["routed"], "routed", "smoke"), false);
  assert.equal(has(["routed", "live"], "routed", "!live"), false);
  // `a,!b` reads "a, or anything without b".
  assert.equal(has(["x"], "a,!b"), true);
  assert.equal(has(["b"], "a,!b"), false);
  // Untagged specs have no tag: they fail an include and pass an exclude.
  assert.equal(has([], "routed"), false);
  assert.equal(has([], "!live"), true);
  assert.equal(has([], " ! live , unit "), true);
});

test("tag syntax is checked where it is written", () => {
  assert.throws(() => parseTagFilter(["routed,"]), /is not a tag or !tag/);
  assert.throws(() => parseTagFilter(["!"]), /is not a tag/);
  assert.throws(() => parseTagFilter(["a b"]), /is not a tag/);
  assert.deepEqual(validateTags(["a", "a", "api:v2", "team/web"], "spec"), ["a", "api:v2", "team/web"]);
  assert.throws(() => validateTags(["!live"], "spec"), /invalid/);
  assert.throws(() => validateTags("live", "spec"), /array/);
  assert.throws(() => spec("bad", { tags: ["has space"] }, async () => {}), /invalid/);
  assert.throws(() => spec.configure({ tags: [","] }), /invalid/);
});

test("file tags merge with spec tags, select, and summarize per tag", async () => {
  const world = createWorld();
  const { events } = installFakeHost(world, { control: { tags: JSON.stringify(["routed,unit", "!live"]) } });
  const ran = [];
  spec.configure({ tags: ["routed"] });
  spec("list devices", { tags: ["smoke"] }, async () => { ran.push("list"); });
  spec("rename device", async () => { ran.push("rename"); throw new Error("boom"); });
  spec("live sync", { tags: ["live"] }, async () => { ran.push("live"); });
  // Another file: no `routed` from this file's configure, so not selected.
  registerOtherFileSpec("elsewhere");

  const report = await run();
  assert.deepEqual(ran, ["list", "rename"]);
  assert.equal(report.filtered, true);
  assert.deepEqual(report.cases.map((c) => [c.id, c.tags]), [
    ["list-devices", ["routed", "smoke"]],
    ["rename-device", ["routed"]],
  ]);
  assert.deepEqual(report.tag_summary.map((row) => [row.tag, row.total, row.passed, row.failed, row.ok]), [
    ["routed", 2, 1, 1, false],
    ["smoke", 1, 1, 0, true],
  ]);
  const started = events.find((event) => event.type === "run_started");
  assert.deepEqual(started.cases.map((c) => c.tags), [["routed", "smoke"], ["routed"]]);
  assert.deepEqual(events.find((event) => event.type === "case_started").tags, ["routed", "smoke"]);

  const junit = renderJUnit(report);
  assert.match(junit, /<property name="tags" value="routed smoke"\/>/);
  assert.match(junit, /<property name="tag:routed" value="total=2 passed=1 failed=1 timeout=0 xpass=0 xfail=0 skipped=0"\/>/);
});

test("an untagged spec shows as (untagged) next to tagged ones", () => {
  const base = { steps: [], assertions: [], attachments: [], covers: [], duration_ms: 1, timeout_ms: 1, name: "", full_name: "" };
  const rows = tagSummary([
    { ...base, id: "a", title: "a", status: "passed", tags: ["live"] },
    { ...base, id: "b", title: "b", status: "timeout", tags: ["live"] },
    { ...base, id: "c", title: "c", status: "passed", tags: [], flaky: true },
  ]);
  assert.deepEqual(rows.map((row) => [row.tag, row.total, row.ok, row.flaky]), [
    ["live", 2, false, 0],
    ["(untagged)", 1, true, 1],
  ]);
  assert.deepEqual(tagSummary([{ ...base, id: "c", title: "c", status: "passed" }]), []);
});

test("the HTML report shows the tag table and each case's tags", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world, { control: {} });
  spec("tagged", { tags: ["unit"] }, async () => {});
  await run();
  const html = Buffer.from(attachments.get("report.html").base64, "base64").toString("utf8");
  assert.match(html, /By tag/);
  assert.match(html, /#unit/);
});

test("the coverage manifest names covered, uncovered and unknown ids", async () => {
  const world = createWorld();
  const manifest = [
    { id: "DEV-1", title: "list devices" },
    { id: "DEV-2", title: "rename a device" },
    { id: "DEV-3" },
    { id: "DEV-4", title: "unselected spec covers it" },
  ];
  const { events } = installFakeHost(world, {
    control: { coversManifest: JSON.stringify(manifest), coversManifestFile: "coverage.yaml", grep: "^(list|rename)" },
  });
  spec("list", { covers: ["DEV-1"] }, async () => {});
  spec("rename", { covers: ["DEV-2", "DEV-9"] }, async () => { throw new Error("nope"); });
  spec("other", { covers: ["DEV-4"] }, async () => {});

  const report = await run();
  const coverage = report.coverage;
  assert.equal(coverage.total, 4);
  assert.equal(coverage.covered, 3);
  assert.equal(coverage.passing, 1);
  assert.equal(coverage.failing, 1);
  assert.deepEqual(coverage.uncovered, [{ id: "DEV-3" }]);
  assert.deepEqual(coverage.unknown, [{ id: "DEV-9", specs: ["rename"] }]);
  const byId = Object.fromEntries(coverage.ids.map((entry) => [entry.id, entry]));
  assert.equal(byId["DEV-1"].status, "passed");
  assert.equal(byId["DEV-2"].status, "failed");
  assert.deepEqual(byId["DEV-4"].specs, [{ id: "other", title: "other", status: "not_run" }]);
  assert.equal(byId["DEV-4"].status, "not_run");
  // The manifest itself stays out of meta; its file name does not.
  assert.equal(report.meta.run.coversManifest, undefined);
  assert.equal(report.meta.run.coversManifestFile, "coverage.yaml");
  const warning = events.find((event) => event.type === "diagnostic" && event.phase === "coverage");
  assert.match(warning.message, /DEV-9/);
});

test("a manifest control is checked", () => {
  assert.equal(parseManifest(undefined), undefined);
  assert.throws(() => parseManifest("{}"), /JSON list/);
  assert.throws(() => parseManifest(JSON.stringify([{ title: "x" }])), /no id/);
  assert.throws(() => parseManifest(JSON.stringify([{ id: "a" }, { id: "a" }])), /twice/);
  const summary = coverageSummary([{ id: "a" }], [], [{ id: "s", title: "S", covers: ["a"] }]);
  assert.equal(summary.ids[0].status, "not_run");
});

test("a bundled registration that maps to no file stops the run instead of losing its tags", async () => {
  const { runInThisContext } = await import("node:vm");
  const register = (mappings) => {
    globalThis.__spec = spec;
    globalThis.__LINGXIA_TEST_SOURCE_MAP__ = { version: 3, sources: ["tests/a.test.ts"], mappings };
    runInThisContext(
      '__spec("placed", async () => {});\n__spec.configure({ tags: ["t"] });\n__spec("lost", async () => {});\n',
      { filename: "lxdev-test://tests" },
    );
  };
  try {
    installFakeHost(createWorld(), { control: {} });
    // Line 3 has no mapping, as a comment line has none.
    register("AAAA;AACA");
    await assert.rejects(run(), /Cannot tell which file registered spec "lost" \(lxdev-test:\/\/tests:3\)/);

    reset();
    const { events } = installFakeHost(createWorld(), { control: {} });
    register("AAAA;AACA;AACA");
    await run();
    const started = events.find((event) => event.type === "run_started");
    assert.deepEqual(started.cases.map((c) => [c.title, c.file, c.tags]), [
      ["placed", "tests/a.test.ts", ["t"]],
      ["lost", "tests/a.test.ts", ["t"]],
    ]);
  } finally {
    delete globalThis.__spec;
    delete globalThis.__LINGXIA_TEST_SOURCE_MAP__;
  }
});
