import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { spec, reset } from "../dist/index.js";
import { failedAt } from "../dist/report.js";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

function coded(code, message, data) {
  return Object.assign(new Error(message), { code, ...(data ? { data } : {}) });
}

function decode(attachments, name) {
  return JSON.parse(Buffer.from(attachments.get(name).base64, "base64").toString("utf8"));
}

for (const code of ["E_PAGE_NOT_ACTIVE", "E_PAGE_NOT_READY"]) {
  test(`a click retries a ${code} rejection whatever its message says`, async () => {
    const world = createWorld();
    const element = world.add({ testId: "save" });
    const query = world.app.page.query;
    let calls = 0;
    world.app.page.query = async (options) => {
      if (++calls <= 2) throw coded(code, "reworded by a newer host");
      return query(options);
    };
    installFakeHost(world);
    spec("mid-transition click", (t) => t.app.page.testId("save").click({ timeout: 1_000, interval: 5 }));

    const protocol = await globalThis.__LINGXIA_TEST__.run();
    assert.equal(protocol.cases[0].status, "passed", JSON.stringify(protocol.cases[0].error));
    assert.equal(element.clicked, 1);
  });
}

test("an element refusal is retried by code; any other coded failure is not", async () => {
  const world = createWorld();
  const element = world.add({ testId: "save" });
  const click = world.app.page.click;
  let calls = 0;
  world.app.page.click = async (options) => {
    if (++calls === 1) throw coded("E_ELEMENT_NOT_INTERACTABLE", "reworded");
    return click(options);
  };
  installFakeHost(world);
  spec("refused once", (t) => t.app.page.testId("save").click({ timeout: 500, interval: 1 }));
  assert.equal((await globalThis.__LINGXIA_TEST__.run()).passed, 1);
  assert.equal(element.clicked, 1);

  reset();
  const other = createWorld();
  other.add({ testId: "save" });
  let dispatched = 0;
  other.app.page.click = async () => {
    dispatched += 1;
    throw coded("E_EVAL_SCRIPT", "JavaScript error: boom");
  };
  installFakeHost(other);
  spec("not retried", { forensics: false }, (t) => t.app.page.testId("save").click({ timeout: 500, interval: 1 }));
  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.cases[0].status, "failed");
  assert.equal(protocol.cases[0].error.code, "E_EVAL_SCRIPT");
  assert.equal(dispatched, 1);
});

test("t.reject and spec.fail pin a driver failure by code", async () => {
  const world = createWorld();
  world.app.page.eval = async () => { throw coded("E_EVAL_SCRIPT", "JavaScript error: boom"); };
  installFakeHost(world);
  spec("rejects by code", async (t) => {
    await t.reject(() => t.automation.lxapp().page.eval({ script: "boom()" }), { code: "E_EVAL_SCRIPT" });
  });
  spec.fail("known failure", { expected: { code: "E_EVAL_SCRIPT" }, forensics: false }, async (t) => {
    await t.automation.lxapp().page.eval({ script: "boom()" });
  });
  spec.fail("different failure", { expected: { code: "E_PAGE_NOT_ACTIVE" }, forensics: false }, async (t) => {
    await t.automation.lxapp().page.eval({ script: "boom()" });
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.deepEqual(protocol.cases.map((c) => c.status), ["passed", "xfail", "failed"]);
  assert.match(protocol.cases[2].error.message, /code "E_PAGE_NOT_ACTIVE", got "E_EVAL_SCRIPT"/);
});

test("a failure names its action, the current page instance and the code that kept it from running", async () => {
  const world = createWorld();
  world.add({ testId: "save" });
  // The app's own navigation replaced the page: every read of it rejects.
  world.app.page.query = async () => {
    throw coded("E_PAGE_NOT_ACTIVE", "page is not active: devices", {
      page: "devices",
      current: { name: "home", path: "pages/home/index", instanceId: "c3d4" },
    });
  };
  const current = world.app.nav.current;
  world.app.nav.current = async () => ({ ...(await current()), instanceId: "a1b2" });
  const { attachments } = installFakeHost(world);
  spec("save", (t) => t.app.page.testId("save").click({ timeout: 300, interval: 5 }));
  spec("no forensics", { forensics: false }, (t) => t.app.page.testId("save").click({ timeout: 300, interval: 5 }));
  spec("asserts", { forensics: false }, (t) => t.expect.poll(() => 1, { timeout: 20 }).toBe(2));
  spec.fail("known inactive page", { expected: { code: "E_PAGE_NOT_ACTIVE" }, forensics: false },
    (t) => t.app.page.testId("save").click({ timeout: 100, interval: 5 }));

  await globalThis.__LINGXIA_TEST__.run();
  const report = decode(attachments, "report.json");
  const [save, noForensics, asserts, known] = report.cases;
  assert.equal(known.status, "xfail", "a timeout caused by a coded rejection carries its code");
  assert.match(save.error.failedAction, /^page\.click .*save/);
  assert.deepEqual(save.error.page, { name: "home", instanceId: "a1b2" });
  assert.deepEqual(noForensics.error.page, { name: "home", instanceId: "c3d4" }, "falls back to the error's data");
  assert.equal(asserts.error.failedAction, undefined);

  assert.equal(report.cases.length, 4, "the nested structure is kept");
  assert.equal(report.failures.length, 3);
  const [first] = report.failures;
  assert.equal(first.id, save.id);
  assert.equal(first.title, "save");
  assert.equal(first.phase, "body");
  assert.equal(first.code, "E_PAGE_NOT_ACTIVE");
  assert.match(first.message, /Timed out after \d+ms waiting to click/);
  assert.match(first.message, /page is not active: devices/);
  assert.equal(first.failedAction, save.error.failedAction);
  assert.deepEqual(first.page, { name: "home", instanceId: "a1b2" });
  assert.match(first.screenshot, /failure\.png$/);
  assert.equal(report.failures[1].screenshot, undefined);
  assert.equal(
    failedAt(save.error),
    `failed at ${save.error.failedAction} on page "home" (#a1b2) — E_PAGE_NOT_ACTIVE`,
  );
});

test("an action the spec expected to reject is not blamed for a later failure", async () => {
  const world = createWorld();
  world.app.page.eval = async () => { throw coded("E_EVAL_SCRIPT", "JavaScript error: boom"); };
  const { attachments } = installFakeHost(world);
  spec("later assertion", { forensics: false }, async (t) => {
    await t.reject(() => t.app.page.eval({ script: "boom()" }), { code: "E_EVAL_SCRIPT" });
    throw new Error("unrelated");
  });

  await globalThis.__LINGXIA_TEST__.run();
  const report = decode(attachments, "report.json");
  assert.equal(report.cases[0].error.message, "unrelated");
  assert.equal(report.cases[0].error.failedAction, undefined);
  assert.equal(failedAt(report.cases[0].error), undefined);
});
