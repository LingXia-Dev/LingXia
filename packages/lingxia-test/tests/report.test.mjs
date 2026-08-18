import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, expect, reset } from "../dist/index.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

function decodeAttachment(attachments, name) {
  const artifact = attachments.get(name);
  assert.ok(artifact, `missing attachment ${name}`);
  return Buffer.from(artifact.base64, "base64").toString("utf8");
}

test("spec.fail inverts only a body assertion", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);

  spec.fail("known broken", async () => {
    expect(1).toBe(2);
  });
  spec.fail("unexpectedly passes", async () => {
    expect(1).toBe(1);
  });
  spec.fail("timeout is not xfail", { timeout: 40 }, async (t) => {
    await new Promise((resolve) => setTimeout(resolve, 200));
    await t.app.eval({ script: "1" });
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.equal(report.cases[0].status, "xfail");
  assert.equal(report.cases[1].status, "xpass");
  assert.equal(report.cases[2].status, "timeout");
  assert.equal(protocol.passed, 1);
  assert.equal(protocol.failed, 2);
});

test("failed specs attach forensics and report.html stays a single inlined file", async () => {
  const world = createWorld();
  world.add({ testId: "home-name", visible: true, value: "" });
  const { attachments } = installFakeHost(world, { logs: "bridge: setData\nconsole: boom" });

  spec("fails for forensics", async (t) => {
    await t.attach("note.txt", "author note");
    expect(false).toBe(true);
  });

  await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  const failed = report.cases[0];
  const paths = failed.attachments.map((item) => item.path);
  assert.ok(paths.includes("attachments/fails-for-forensics/failure.png"));
  assert.ok(paths.includes("attachments/fails-for-forensics/forensics.json"));
  assert.ok(paths.includes("attachments/fails-for-forensics/logs.txt"));
  assert.ok(paths.includes("attachments/fails-for-forensics/note.txt"));
  assert.equal(report.partial, false);

  const html = decodeAttachment(attachments, "report.html");
  assert.match(html, /<!DOCTYPE html>/);
  assert.doesNotMatch(html, /cdn\.|unpkg|jsdelivr|https:\/\/fonts/);
  assert.match(html, /data:image\/png;base64,/);
  assert.match(html, /attachments\/fails-for-forensics\/failure\.png/);
});

test("omits the log tail when the host has no ring", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);

  spec("fails without logs", async () => {
    expect(1).toBe(2);
  });

  await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  const names = report.cases[0].attachments.map((item) => item.name);
  assert.ok(names.includes("failure.png"));
  assert.ok(names.includes("forensics.json"));
  assert.ok(!names.includes("logs.txt"));
});

test("t.expect.poll retries an arbitrary read", async () => {
  const world = createWorld();
  installFakeHost(world);
  let value = 0;
  spec("polls", async (t) => {
    setTimeout(() => {
      value = 3;
    }, 30);
    await t.expect.poll(() => value, { timeout: 400 }).toBe(3);
  });
  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.passed, 1);
});
