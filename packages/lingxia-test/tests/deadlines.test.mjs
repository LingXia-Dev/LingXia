import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, reset } from "../dist/index.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

const hang = () => new Promise(() => {});

async function runOne() {
  const protocol = await globalThis.__LINGXIA_TEST__.run();
  return protocol.cases[0];
}

test("a query that never returns fails the click by name, not the spec", async () => {
  const world = createWorld();
  world.add({ testId: "save" });
  world.app.page.query = hang;
  installFakeHost(world);

  spec("hung query", { timeout: 5_000, forensics: false }, (t) => t.app.view.testId("save").click({ timeout: 120 }));

  const started = Date.now();
  const result = await runOne();
  assert.ok(Date.now() - started < 2_000, "the action budget, not the spec budget, bounded the call");
  assert.equal(result.status, "failed");
  assert.match(result.error.message, /page\.query did not return within \d+ms/);
  assert.match(result.error.message, /while trying to click "\[data-testid=\\"save\\"\]"/);
  assert.match(result.error.message, /deadlines\.test\.mjs:\d+:\d+/);
  assert.equal(result.steps[0].name, "page.click");
  assert.equal(result.steps[0].status, "timeout");
});

test("a click that never returns fails the click by name", async () => {
  const world = createWorld();
  world.add({ testId: "save" });
  world.app.page.click = hang;
  installFakeHost(world);

  spec("hung dispatch", { timeout: 5_000, forensics: false }, (t) => t.app.view.testId("save").click({ timeout: 120 }));

  const result = await runOne();
  assert.equal(result.status, "failed");
  assert.match(result.error.message, /page\.click did not return/);
});

test("a t.expect(fn) read that never returns names the assertion", async () => {
  installFakeHost(createWorld());

  spec("hung read", { timeout: 5_000, forensics: false }, async (t) => {
    await t.step("wait for sync", async () => {
      await t.expect(hang, { timeout: 120 }).toBe(1);
    });
  });

  const result = await runOne();
  assert.equal(result.status, "failed");
  assert.match(result.error.message, /t\.expect\(fn\) read did not return within \d+ms/);
  assert.match(result.error.message, /while retrying t\.expect\(fn\) toBe/);
  assert.match(result.error.message, /in step "wait for sync"/);
});

test("a locator read that never returns fails the locator assertion", async () => {
  const world = createWorld();
  world.app.page.query = hang;
  installFakeHost(world);

  spec("hung locator read", { timeout: 5_000, forensics: false }, (t) =>
    t.expect(t.app.view.testId("banner")).toBeVisible({ timeout: 120 }));

  const result = await runOne();
  assert.equal(result.status, "failed");
  assert.match(result.error.message, /locator read did not return/);
  assert.match(result.error.message, /while retrying toBeVisible/);
});

test("an assertion timeout longer than the spec's remaining budget is clamped, visibly", async () => {
  installFakeHost(createWorld());

  spec("clamped assertion", { timeout: 400, forensics: false }, (t) =>
    t.expect(t.app.view.testId("absent")).toBeVisible({ timeout: 5_000 }));

  const result = await runOne();
  assert.equal(result.status, "failed", "the assertion, not the spec timer, ends the spec");
  assert.match(result.error.message, /toBeVisible/);
  assert.match(result.error.message, /timeout 5000ms was clamped to \d+ms, the time left in the spec's budget/);
});

test("an action timeout longer than the spec's remaining budget is clamped, visibly", async () => {
  installFakeHost(createWorld());

  spec("clamped click", { timeout: 400, forensics: false }, (t) =>
    t.app.view.testId("absent").click({ timeout: 5_000 }));

  const result = await runOne();
  assert.equal(result.status, "failed");
  assert.match(result.error.message, /waiting to click/);
  assert.match(result.error.message, /timeout 5000ms was clamped/);
});

test("a timeout within the spec's budget is not clamped", async () => {
  installFakeHost(createWorld());

  spec("unclamped", { timeout: 5_000, forensics: false }, (t) =>
    t.expect(() => 0, { timeout: 80 }).toBe(1));

  const result = await runOne();
  assert.equal(result.status, "failed");
  assert.doesNotMatch(result.error.message, /clamped/);
});

for (const [code, transient] of [
  ["E_PAGE_NOT_ACTIVE", "page is not active: detail"],
  ["E_PAGE_NOT_READY", "page WebView is not ready"],
  ["E_PAGE_NOT_READY", "WebView error: No current page"],
]) {
  test(`a click retries while a navigation reports ${code} "${transient}"`, async () => {
    const world = createWorld();
    const element = world.add({ testId: "save" });
    const query = world.app.page.query;
    let calls = 0;
    world.app.page.query = async (options) => {
      calls += 1;
      if (calls <= 3) throw Object.assign(new Error(transient), { code });
      return query(options);
    };
    installFakeHost(world);

    spec("mid-transition click", (t) => t.app.view.testId("save").click({ timeout: 1_000, interval: 5 }));

    const result = await runOne();
    assert.equal(result.status, "passed", JSON.stringify(result.error));
    assert.equal(element.clicked, 1);
    assert.ok(calls > 3);
  });
}

test("a page error is transient by its code, not its message", async () => {
  const world = createWorld();
  world.add({ testId: "save" });
  let calls = 0;
  world.app.page.query = async () => {
    calls += 1;
    throw Object.assign(new Error("page is not active: detail"), { code: "E_AUTOMATION" });
  };
  installFakeHost(world);

  spec("uncoded", { forensics: false }, (t) => t.app.view.testId("save").click({ timeout: 1_000, interval: 5 }));

  const result = await runOne();
  assert.equal(result.status, "failed");
  assert.equal(calls, 1);
});

test("a transient error that outlasts the budget is reported as the reason", async () => {
  const world = createWorld();
  world.app.page.query = async () => {
    throw Object.assign(new Error("page is not active: detail"), { code: "E_PAGE_NOT_ACTIVE" });
  };
  installFakeHost(world);

  spec("never lands", { forensics: false }, (t) => t.app.view.testId("save").click({ timeout: 100, interval: 5 }));

  const result = await runOne();
  assert.equal(result.status, "failed");
  assert.match(result.error.message, /Timed out after \d+ms waiting to click/);
  assert.match(result.error.message, /page not ready: page is not active: detail/);
});

test("a non-transient query error still fails the action at once", async () => {
  const world = createWorld();
  let calls = 0;
  world.app.page.query = async () => {
    calls += 1;
    throw new Error("SyntaxError: invalid selector");
  };
  installFakeHost(world);

  spec("bad selector", { forensics: false }, (t) => t.app.view.css("[").click({ timeout: 1_000, interval: 5 }));

  const started = Date.now();
  const result = await runOne();
  assert.equal(result.status, "failed");
  assert.match(result.error.message, /invalid selector/);
  assert.equal(calls, 1);
  assert.ok(Date.now() - started < 500);
});

test("a pre-dispatch page error is retried, a WebView2 dispatch error is not", async () => {
  const world = createWorld();
  const element = world.add({ testId: "save" });
  const click = world.app.page.click;
  let calls = 0;
  world.app.page.click = async (options) => {
    calls += 1;
    if (calls === 1) throw Object.assign(new Error("page is not active: detail"), { code: "E_PAGE_NOT_ACTIVE" });
    return click(options);
  };
  installFakeHost(world);
  spec("retried", (t) => t.app.view.testId("save").click({ timeout: 1_000, interval: 5 }));
  assert.equal((await runOne()).status, "passed");
  assert.equal(element.clicked, 1);

  reset();
  let dispatched = 0;
  world.app.page.click = async () => {
    dispatched += 1;
    throw new Error("ExecuteScript failed: 0x8007139F");
  };
  installFakeHost(world);
  spec("ambiguous", { forensics: false }, (t) => t.app.view.testId("save").click({ timeout: 1_000, interval: 5 }));
  assert.equal((await runOne()).status, "failed");
  assert.equal(dispatched, 1, "an ambiguous dispatch failure must not resubmit the input");
});

test("waitFor retries a page that is not ready yet", async () => {
  const world = createWorld();
  world.add({ testId: "sheet" });
  const query = world.app.page.query;
  let calls = 0;
  world.app.page.query = async (options) => {
    calls += 1;
    if (calls <= 2) throw Object.assign(new Error("page WebView is not ready"), { code: "E_PAGE_NOT_READY" });
    return query(options);
  };
  installFakeHost(world);

  spec("waits across the transition", (t) => t.app.view.testId("sheet").waitFor({ timeout: 1_000, interval: 5 }));

  assert.equal((await runOne()).status, "passed");
});
