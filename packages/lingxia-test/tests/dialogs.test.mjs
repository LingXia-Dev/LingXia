import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, expect } from "../dist/index.js";
import { reset } from "../dist/runner.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

const finished = (events) => events.filter((event) => event.type === "case_finished");

test("toasts are observed per spec and read through expect.poll + objectContaining", async () => {
  const world = createWorld();
  // The app's Logic shows the toast a moment after the tap, as a save would.
  world.add({ testId: "save", text: "Save", onClick: () => setTimeout(() => world.dialogs.logic.showToast({ title: "Saved", icon: "success" }), 20) });
  const { events } = installFakeHost(world);
  let second;

  spec("shows a toast", async (t) => {
    await t.app.view.testId("save").click();
    await expect.poll(() => t.app.dialogs.toasts()).toContainEqual(expect.objectContaining({ title: "Saved", icon: "success" }));
    const [toast] = await t.app.dialogs.toasts();
    expect(toast.duration).toBe(1500);
    expect(typeof toast.at).toBe("number");
  });
  spec("starts with none", async (t) => { second = await t.app.dialogs.toasts(); });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.passed, 2, JSON.stringify(finished(events)));
  assert.deepEqual(second, [], "each spec starts with nothing recorded");
  assert.equal(world.dialogs.watching, false, "the watch ends with the spec");
});

test("a queued answer answers the next modal, and every modal is recorded", async () => {
  const world = createWorld();
  const answers = [];
  world.add({ testId: "delete", onClick: () => answers.push(world.dialogs.logic.showModal({ title: "Delete?", content: "It cannot be undone.", confirmText: "Delete" })) });
  const { events } = installFakeHost(world);
  let modals;

  spec("confirms a delete", async (t) => {
    await t.app.dialogs.answerNextModal({ confirm: true });
    await t.app.dialogs.answerNextModal({ confirm: false });
    await t.app.view.testId("delete").click();
    await t.app.view.testId("delete").click();
    modals = await t.app.dialogs.modals();
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.passed, 1, JSON.stringify(finished(events)));
  assert.deepEqual(answers, [{ confirm: true }, { confirm: false }]);
  assert.deepEqual(modals, [
    { title: "Delete?", content: "It cannot be undone.", confirmText: "Delete", answer: { confirm: true } },
    { title: "Delete?", content: "It cannot be undone.", confirmText: "Delete", answer: { confirm: false } },
  ]);
});

test("once answering, a modal with no answer queued fails the spec at once, naming it; the run goes on", async () => {
  const world = createWorld();
  world.add({ testId: "delete", onClick: () => { try { world.dialogs.logic.showModal({ title: "Delete?", content: "Gone for good." }); } catch {} } });
  const { events } = installFakeHost(world);
  let next = false;

  spec("answers one modal, then meets a second", { timeout: 20_000, forensics: false }, async (t) => {
    await t.app.dialogs.answerNextModal({ confirm: true });
    await t.app.view.testId("delete").click();
    await t.app.view.testId("delete").click();
    // A wait that would otherwise run for most of the budget.
    await expect(t.app.view.testId("never")).toBeVisible({ timeout: 15_000 });
  });
  spec("runs after it", () => { next = true; });

  const started = Date.now();
  const protocol = await globalThis.__LINGXIA_TEST__.run();
  const [failed] = finished(events);
  assert.equal(failed.status, "failed");
  assert.match(failed.error.message, /^Unanswered dialog: a modal appeared with no answer queued: title "Delete\?", content "Gone for good\."/);
  assert.ok(Date.now() - started < 5_000, "the spec fails when the modal appears, not at its timeout");
  assert.equal(next, true);
  assert.equal(protocol.passed, 1);
});

test("a spec that queues no answer sees modals and action sheets drawn and recorded, and passes", async () => {
  const world = createWorld();
  // The page draws the modal; the spec taps its buttons like a user.
  let drawn;
  world.add({ testId: "sign-out", onClick: () => { drawn = world.dialogs.logic.showModal({ title: "Sign out?", content: "", confirmText: "Sign out" }); } });
  world.add({ testId: "confirm", onClick: () => drawn.close(true) });
  world.add({ testId: "more", onClick: () => { drawn = world.dialogs.logic.showActionSheet(["Edit", "Delete"]); } });
  world.add({ testId: "delete-item", onClick: () => drawn.close({ index: 1 }) });
  const { events } = installFakeHost(world);
  let modals, sheets;

  spec("taps the drawn dialogs", async (t) => {
    await t.app.view.testId("sign-out").click();
    expect(drawn.drawn).toBe(true);
    await t.app.view.testId("confirm").click();
    await t.app.view.testId("more").click();
    await t.app.view.testId("delete-item").click();
    modals = await t.app.dialogs.modals();
    sheets = await t.app.dialogs.actionSheets();
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.passed, 1, JSON.stringify(finished(events)));
  assert.deepEqual(modals, [{ title: "Sign out?", content: "", confirmText: "Sign out", drawn: true, answer: { confirm: true } }]);
  assert.deepEqual(sheets, [{ items: ["Edit", "Delete"], drawn: true, answer: { index: 1 } }]);
});

test("queuing a modal answer answers modals from then on; action sheets stay drawn", async () => {
  const world = createWorld();
  const seen = [];
  world.add({ testId: "ask", onClick: () => { try { seen.push(world.dialogs.logic.showModal({ title: "Ask" })); } catch (error) { seen.push(error.message); } } });
  world.add({ testId: "more", onClick: () => seen.push(world.dialogs.logic.showActionSheet(["A"])) });
  const { events } = installFakeHost(world);

  spec("drawn, then answered", async (t) => {
    await t.app.view.testId("ask").click();       // before any answer: drawn
    await t.app.dialogs.answerNextModal({ confirm: false });
    await t.app.view.testId("ask").click();       // answered
    await t.app.view.testId("more").click();      // sheets are not answering
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.passed, 1, JSON.stringify(finished(events)));
  assert.equal(seen[0].drawn, true);
  assert.deepEqual(seen[1], { confirm: false });
  assert.equal(seen[2].drawn, true);
});

test("an answer no dialog used fails the spec when it ends", async () => {
  const world = createWorld();
  const { events } = installFakeHost(world);

  spec("expects a modal that never comes", async (t) => {
    await t.app.dialogs.answerNextModal({ confirm: true });
    await t.app.dialogs.answerNextActionSheet({ index: 0 });
  });

  await globalThis.__LINGXIA_TEST__.run();
  const [failed] = finished(events);
  assert.equal(failed.status, "failed");
  assert.match(failed.error.message,
    /1 modal answer \(t\.app\.dialogs\.answerNextModal\) and 1 action sheet answer \(t\.app\.dialogs\.answerNextActionSheet\) queued but no dialog appeared to use them/);
});

test("action sheets take an index or a cancel; once answering, an unanswered one fails the spec", async () => {
  const world = createWorld();
  const picked = [];
  world.add({ testId: "more", onClick: () => { try { picked.push(world.dialogs.logic.showActionSheet(["Edit", "Delete"])); } catch (error) { picked.push(error.message); } } });
  const { events } = installFakeHost(world);
  let sheets;

  spec("picks and cancels", async (t) => {
    await t.app.dialogs.answerNextActionSheet({ index: 1 });
    await t.app.dialogs.answerNextActionSheet({ cancel: true });
    await t.app.view.testId("more").click();
    await t.app.view.testId("more").click();
    sheets = await t.app.dialogs.actionSheets();
  });
  spec("runs out of answers", { forensics: false }, async (t) => {
    await t.app.dialogs.answerNextActionSheet({ index: 0 });
    await t.app.view.testId("more").click();
    await t.app.view.testId("more").click();
    await new Promise((resolve) => setTimeout(resolve, 2_000));
  });

  await globalThis.__LINGXIA_TEST__.run();
  const [passed, failed] = finished(events);
  assert.equal(passed.status, "passed", JSON.stringify(passed.error));
  assert.deepEqual(picked.slice(0, 2), [{ index: 1 }, { cancel: true }]);
  assert.deepEqual(sheets, [
    { items: ["Edit", "Delete"], answer: { index: 1 } },
    { items: ["Edit", "Delete"], answer: { cancel: true } },
  ]);
  assert.equal(failed.status, "failed");
  assert.match(failed.error.message, /Unanswered dialog: an action sheet appeared with no answer queued: items \["Edit","Delete"\]/);
});

test("answers are checked before they are queued", async () => {
  const world = createWorld();
  const { events } = installFakeHost(world);

  spec("bad answers", async (t) => {
    await assert.rejects(t.app.dialogs.answerNextModal({ confirm: "yes" }), /answerNextModal takes \{ confirm: true \| false \}/);
    await assert.rejects(t.app.dialogs.answerNextActionSheet({ index: -1 }), /answerNextActionSheet takes \{ index \} or \{ cancel: true \}/);
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.passed, 1, JSON.stringify(finished(events)));
});

test("a host without dialog watching runs specs as before", async () => {
  const world = createWorld();
  delete world.app.dialogs;
  const { events } = installFakeHost(world);
  spec("no dialogs driver", async () => {});
  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.passed, 1, JSON.stringify(finished(events)));
  assert.equal(events.some((event) => event.phase === "dialogs"), false);
});


for (const failing of ['watch', 'unanswered', 'unwatch']) {
  test(`dialog ${failing} failures cannot produce a passed spec`, async () => {
    const world = createWorld();
    world.app.dialogs[failing] = () => { throw new Error(`${failing} unavailable`); };
    installFakeHost(world);
    let ran = false;
    spec('dialog failure', { forensics: false }, async () => {
      ran = true;
      await new Promise(resolve => setTimeout(resolve, 20));
    });
    const report = await globalThis.__LINGXIA_TEST__.run();
    assert.equal(report.cases[0].status, 'failed');
    assert.match(report.cases[0].error.message, new RegExp(`${failing} unavailable`));
    if (failing === 'watch') assert.equal(ran, false);
  });
}
