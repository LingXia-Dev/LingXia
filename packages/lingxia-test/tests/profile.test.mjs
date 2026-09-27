import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, reset, run } from "../dist/index.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

/** Mirrors the host: an isolated app's data, closed-copy checkpoints. */
function fakeProfile(world, options = {}) {
  const profile = {
    data: { value: 0 },
    checkpoints: new Map(),
    calls: [],
    async checkpoint() {
      profile.calls.push(["checkpoint", world.navCalls.length]);
      const id = `cp-${profile.checkpoints.size + 1}`;
      profile.checkpoints.set(id, structuredClone(profile.data));
      return id;
    },
    async restore(id, restoreOptions) {
      profile.calls.push(restoreOptions === undefined ? ["restore", id] : ["restore", id, restoreOptions]);
      if (options.failRestore) throw Object.assign(new Error("reopen failed"), { code: "E_AUTOMATION" });
      // Keys the globs match keep their current state (exact names here).
      const keep = restoreOptions?.keep ?? [];
      const current = profile.data;
      profile.data = structuredClone(profile.checkpoints.get(id));
      for (const key of keep) {
        if (key in current) profile.data[key] = current[key];
        else delete profile.data[key];
      }
      if (options.oldHost) return undefined;
      return { kept: keep.filter((key) => key in current) };
    },
    async drop(id) {
      profile.calls.push(["drop", id]);
      profile.checkpoints.delete(id);
    },
  };
  world.app.profile = profile;
  return profile;
}

test("restoreProfile rolls a spec's writes back before the next spec", async () => {
  const world = createWorld();
  const profile = fakeProfile(world);
  installFakeHost(world);
  let seenByBodyDefer;
  let seenBySecond;

  spec("writes", { restoreProfile: true }, async (t) => {
    profile.data.value = 42;
    t.defer(() => { seenByBodyDefer = profile.data.value; });
  });
  spec("reads", async () => {
    seenBySecond = profile.data.value;
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(seenByBodyDefer, 42, "the spec's own cleanup runs before the rollback");
  assert.equal(seenBySecond, 0);
  assert.deepEqual(profile.calls.map(([name]) => name), ["checkpoint", "restore", "drop"]);
  assert.equal(profile.checkpoints.size, 0);
  // The checkpoint is taken before the implied relaunch.
  assert.equal(profile.calls[0][1], 0);
  assert.ok(world.navCalls.some(([name]) => name === "relaunch"), "restoreProfile implies fresh");
  const traced = report.cases[0].steps.map((step) => step.name);
  assert.ok(traced.includes("profile.checkpoint") && traced.includes("profile.restore"));
});

test("a failed rollback stops the run instead of leaking data", async () => {
  const world = createWorld();
  const profile = fakeProfile(world, { failRestore: true });
  installFakeHost(world);
  let secondRan = false;

  spec("writes", { restoreProfile: true }, async () => {});
  spec("would see stale data", async () => { secondRan = true; });

  const report = await run();
  assert.equal(secondRan, false);
  assert.equal(report.partial, true);
  assert.equal(report.cases[0].status, "failed");
  assert.equal(report.cases[0].error.phase, "defer");
  assert.equal(report.cases[1].status, "skipped");
  assert.match(report.cases[1].reason, /restoreProfile/);
  assert.equal(profile.checkpoints.size, 0, "the failed rollback's checkpoint is dropped too");
});

test("restoreProfile outside an isolated run fails before the body", async () => {
  const world = createWorld();
  world.app.profile = {
    async checkpoint() {
      throw Object.assign(new Error("profile rollback needs an isolated run (lxdev test --profile)"),
        { code: "E_PROFILE_NOT_ISOLATED" });
    },
  };
  installFakeHost(world);
  let bodyRan = false;

  spec("needs isolation", { restoreProfile: true }, async () => { bodyRan = true; });

  const report = await run();
  assert.equal(bodyRan, false);
  assert.equal(report.cases[0].status, "failed");
  assert.equal(report.cases[0].error.phase, "beforeEach");
  assert.equal(report.cases[0].error.code, "E_PROFILE_NOT_ISOLATED");
});

test("t.app.profile re-selects the app after a switch", async () => {
  const world = createWorld();
  fakeProfile(world);
  let selections = 0;
  installFakeHost(world);
  const automation = globalThis.lx.automation;
  globalThis.lx.automation = () => {
    const root = automation();
    return { ...root, lxapp: (...args) => { selections += 1; return root.lxapp(...args); } };
  };

  spec("manual", async (t) => {
    const before = selections;
    const checkpoint = await t.app.profile.checkpoint();
    assert.deepEqual(checkpoint, { id: "cp-1" });
    assert.deepEqual(await t.app.profile.restore(checkpoint), { kept: [] });
    assert.equal(await t.app.profile.drop(checkpoint), undefined);
    assert.equal(selections - before, 2, "checkpoint and restore each re-select the app");
    // The id alone still names it.
    const again = await t.app.profile.checkpoint();
    await t.app.profile.restore(again.id);
    await t.app.profile.drop(again.id);
    await t.reject(() => t.app.profile.restore({}), { message: /needs the checkpoint t\.app\.profile\.checkpoint\(\) resolved/ });
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
});

test("a saved t.app, and any part of it, follows the app a profile switch reopened", async () => {
  const world = createWorld();
  const profile = fakeProfile(world);
  world.add({ testId: "save" });
  installFakeHost(world);
  // Each selection is a driver of the running instance; a switch reopens the
  // app as a new instance, and the old drivers stop reaching it.
  let instance = 1;
  const reopen = (op) => async (...args) => { const result = await op(...args); instance += 1; return result; };
  profile.checkpoint = reopen(profile.checkpoint);
  profile.restore = reopen(profile.restore);
  const automation = globalThis.lx.automation;
  globalThis.lx.automation = () => {
    const root = automation();
    const lxapp = (...args) => {
      const selected = instance;
      const live = (value) => new Proxy(value, {
        get(target, prop) {
          const member = Reflect.get(target, prop, target);
          if (typeof member === "function") {
            return (...callArgs) => {
              if (selected !== instance && prop !== "checkpoint" && prop !== "restore") {
                return Promise.reject(Object.assign(new Error(`instance ${selected} is closed`), { code: "E_AUTOMATION" }));
              }
              return member.apply(target, callArgs);
            };
          }
          return member && typeof member === "object" ? live(member) : member;
        },
      });
      return live(root.lxapp(...args));
    };
    return { ...root, lxapp };
  };
  const seen = {};

  spec("follows", async (t) => {
    const app = t.app;
    const { view, logic, nav } = app;
    const save = view.testId("save");
    seen.same = t.automation.lxapp("demo-app") === app;
    const checkpoint = await app.profile.checkpoint();
    await save.click();
    await nav.current();
    await view.screenshot();
    await app.info();
    seen.data = await logic.eval(() => 1);
    await t.automation.lxapp("demo-app").profile.restore(checkpoint);
    await save.click();
    seen.instance = instance;
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(seen.same, true, "t.automation.lxapp(id) of the pinned app is t.app");
  assert.equal(seen.instance, 3);
});

test("restoreProfile keep carries chosen keys over the rollback", async () => {
  const world = createWorld();
  const profile = fakeProfile(world);
  profile.data.token = "t0";
  installFakeHost(world);
  let seen;

  spec("rotates the token", { restoreProfile: { keep: ["token"] } }, async () => {
    profile.data.value = 42;
    profile.data.token = "t1";
  });
  spec("reads", async () => {
    seen = { ...profile.data };
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.deepEqual(seen, { value: 0, token: "t1" });
  assert.deepEqual(profile.calls[1], ["restore", "cp-1", { keep: ["token"] }]);
  const restore = report.cases[0].steps.find((step) => step.name === "profile.restore");
  assert.equal(restore.detail, "cp-1 keep token");
});

test("restoreProfile rejects a malformed keep at registration", () => {
  assert.throws(() => spec("bad", { restoreProfile: { keep: "auth.*" } }, async () => {}), TypeError);
  assert.throws(() => spec("bad", { restoreProfile: { keep: [""] } }, async () => {}), TypeError);
  assert.throws(() => spec("bad", { restoreProfile: {} }, async () => {}), TypeError);
});
