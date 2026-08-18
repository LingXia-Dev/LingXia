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

test("report json and html include run metadata, steps, expected/actual, and no mojibake", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world, {
    args: { platform: "windows", framework: "react" },
  });

  spec("steps and a matcher failure", { id: "UNIT-REPORT-001", covers: ["lx.demo"] }, async (t) => {
    await t.step("record a passing step", async () => {
      expect(1).toBe(1);
    });
    expect(1).toBe(2);
  });
  spec.skip("pending backlog hole", {
    id: "PEND-UNIT-001",
    covers: ["lx.share"],
    reason: "OS share sheet cannot be driven without a device-lab helper",
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 1);
  assert.equal(protocol.skipped, 1);

  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.equal(report.meta.platform, "windows");
  assert.equal(report.meta.framework, "react");
  assert.equal(report.meta.args.platform, "windows");
  assert.ok(typeof report.meta.started_at === "string" && report.meta.started_at.includes("T"));
  assert.equal(report.cases[0].id, "UNIT-REPORT-001");
  assert.equal(report.cases[0].steps.length, 1);
  assert.equal(report.cases[0].error.expected, "2");
  assert.equal(report.cases[0].error.actual, "1");
  const passingAssert = report.cases[0].steps[0].assertions.find((item) => item.passed);
  assert.ok(passingAssert);
  assert.equal(passingAssert.matcher, "toBe");
  assert.equal(passingAssert.expected, "1");
  assert.equal(passingAssert.actual, "1");
  const failingAssert = report.cases[0].assertions.find((item) => !item.passed);
  assert.ok(failingAssert);
  assert.equal(failingAssert.expected, "2");
  assert.equal(failingAssert.actual, "1");
  assert.equal(report.cases[1].status, "skipped");
  assert.match(report.cases[1].reason, /device-lab/);

  const html = decodeAttachment(attachments, "report.html");
  assert.match(html, /<meta charset="utf-8">/);
  assert.match(html, /windows/);
  assert.match(html, /react/);
  assert.match(html, /UNIT-REPORT-001/);
  assert.match(html, /record a passing step/);
  assert.match(html, /<th>expected<\/th>/);
  assert.match(html, /<th>actual<\/th>/);
  assert.match(html, /device-lab helper/);
  assert.match(html, /&middot;/);
  assert.doesNotMatch(html, /Â·/);
  assert.doesNotMatch(html, /\u00B7/);
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
