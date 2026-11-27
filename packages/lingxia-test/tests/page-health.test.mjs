import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { test as lxTest } from "../dist/index.js";
import { reset, run } from "../dist/runner.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

/** A host page-event ring; `beforeRead(n)` runs before the n-th read answers. */
function pageEvents(beforeRead = () => {}) {
  const events = [];
  let seq = 0;
  let reads = 0;
  return {
    push(event) {
      events.push({ seq: ++seq, atMs: 0, appId: "app", page: "pages/home", kind: "page_failed", message: "failed", ...event });
    },
    async read({ since = 0 } = {}) {
      beforeRead(++reads, this);
      return { events: events.filter((event) => event.seq > since), dropped: 0 };
    },
  };
}

test("an undeclared page failure fails the test that saw it", async () => {
  const ring = pageEvents();
  installFakeHost(createWorld(), { pageEvents: ring });
  lxTest("breaks", async () => { ring.push({ code: "UI_TIMEOUT", message: "render timed out" }); });
  lxTest("clean", async () => {});
  const report = await run();
  const [broken, clean] = report.cases;
  assert.equal(broken.status, "failed");
  assert.equal(broken.error.code, "E_PAGE_HEALTH");
  assert.equal(broken.error.phase, "page");
  assert.match(broken.error.message, /page_failed UI_TIMEOUT on app pages\/home: render timed out/);
  assert.equal(broken.page_health.events[0].expected, false);
  assert.equal(clean.status, "passed");
  assert.equal(clean.page_health, undefined);
  assert.deepEqual({ events: report.page_health.events, cases: report.page_health.cases }, { events: 1, cases: 1 });
});

test("expectPageFailure declares the failure; every event must match and one must occur", async () => {
  const ring = pageEvents();
  installFakeHost(createWorld(), { pageEvents: ring });
  lxTest("declared", { expectPageFailure: /UI_TIMEOUT/ }, async () => { ring.push({ code: "UI_TIMEOUT" }); });
  lxTest("never failed", { expectPageFailure: "UI_TIMEOUT" }, async () => {});
  lxTest("other failure too", { expectPageFailure: { code: "UI_TIMEOUT", page: "pages/home" } }, async () => {
    ring.push({ code: "UI_TIMEOUT" });
    ring.push({ kind: "webview_crashed", page: "pages/web", message: "process gone" });
  });
  const report = await run();
  const [declared, never, other] = report.cases;
  assert.equal(declared.status, "passed", JSON.stringify(declared.error));
  assert.equal(declared.page_health.events[0].expected, true);
  assert.equal(never.status, "failed");
  assert.match(never.error.message, /expectPageFailure UI_TIMEOUT: no page failed during the test/);
  assert.equal(other.status, "failed");
  assert.match(other.error.message, /webview_crashed on app pages\/web: process gone/);
  assert.match(other.error.message, /does not match them/);
  assert.equal(report.page_health.events, 1);
});

test("test.fail never absorbs a page failure", async () => {
  const ring = pageEvents();
  installFakeHost(createWorld(), { pageEvents: ring });
  lxTest.fail("known bug", async () => {
    ring.push({ code: "UI_TIMEOUT" });
    throw new Error("the known bug");
  });
  lxTest.fail("passes but breaks a page", async () => { ring.push({ code: "UI_TIMEOUT" }); });
  const report = await run();
  const [known, xpass] = report.cases;
  assert.equal(known.status, "failed");
  assert.equal(known.error.code, "E_PAGE_HEALTH");
  assert.equal(xpass.status, "failed");
  assert.equal(xpass.error.code, "E_PAGE_HEALTH");
});

test("pageHealth turns a check off; its events are recorded, not graded", async () => {
  const ring = pageEvents();
  installFakeHost(createWorld(), { pageEvents: ring });
  lxTest("stalls on purpose", { pageHealth: { frameGap: false } }, async () => {
    ring.push({ kind: "frame_gap", gapMs: 1800, message: "no frame for 1800 ms" });
  });
  lxTest("still sees blanks", { pageHealth: { frameGap: false } }, async () => {
    ring.push({ kind: "blank_page", message: "one colour" });
  });
  const report = await run();
  const [stall, blank] = report.cases;
  assert.equal(stall.status, "passed", JSON.stringify(stall.error));
  assert.deepEqual(stall.page_health.opted_out, ["frameGap"]);
  assert.deepEqual(stall.page_health.checks, ["events", "blank"]);
  assert.equal(stall.page_health.events[0].opted_out, true);
  assert.equal(stall.page_health.events[0].gap_ms, 1800);
  assert.equal(blank.status, "failed");
});

test("a page failure between tests names the test before it", async () => {
  // Reads: before the first test, grading "first", before "second".
  const ring = pageEvents((n, self) => { if (n === 3) self.push({ code: "UI_TIMEOUT", message: "late" }); });
  const { events } = installFakeHost(createWorld(), { pageEvents: ring });
  lxTest("first", async () => {});
  lxTest("second", async () => {});
  const report = await run();
  assert.deepEqual(report.cases.map((entry) => entry.status), ["passed", "passed"]);
  assert.equal(report.page_health.outside.length, 1);
  assert.equal(report.page_health.outside[0].after, "first");
  const note = events.find((event) => event.type === "diagnostic" && event.phase === "page_health");
  assert.match(note.message, /after "first" finished, before "second" started; the run cannot pass/);
});

test("a host without page events grades nothing", async () => {
  installFakeHost(createWorld());
  lxTest("plain", { expectPageFailure: /UI_TIMEOUT/ }, async () => {});
  const report = await run();
  assert.equal(report.cases[0].status, "passed");
  assert.equal(report.page_health, undefined);
});

test("malformed page health options are refused at registration", () => {
  assert.throws(() => lxTest("bad", { expectPageFailure: 3 }, async () => {}), /expectPageFailure must be/);
  assert.throws(() => lxTest("bad", { pageHealth: { frameGap: true } }, async () => {}), /pageHealth takes only/);
});
