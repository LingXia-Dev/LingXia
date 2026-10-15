import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, expect, CaptureUnavailableError } from "../dist/index.js";
import { reset } from "../dist/runner.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

function report(attachments) {
  return JSON.parse(Buffer.from(attachments.get("report.json").base64, "base64").toString("utf8"));
}

function coded(code, message) {
  return Object.assign(new Error(message), { code });
}

test("app and device captures ask the host by path and attach the file without bytes", async () => {
  const { attachments } = installFakeHost(createWorld());
  const asked = [];
  const host = globalThis.__LINGXIA_AUTOMATION_HOST__;
  host.appScreenshot = async (name, ...surface) => {
    asked.push(["app", name, ...surface]);
    return { path: name, width: 2560, height: 1600, scale: 2 };
  };
  host.deviceScreenshot = async (name) => {
    asked.push(["device", name]);
    return { path: name, width: 1080, height: 2400, scale: 2.75 };
  };
  let shots;
  spec("captures", { id: "SHOT", forensics: false }, async (t) => {
    const app = await t.app.screenshot();
    const aside = await t.app.screenshot({ surface: "aside-1" });
    const device = await t.step("device", () => t.device.screenshot());
    shots = [app, aside, device];
    expect(device.height > device.width).toBe(true);
  });

  await globalThis.__LINGXIA_TEST__.run();
  const record = report(attachments).cases[0];
  assert.equal(record.status, "passed", JSON.stringify(record.error));
  const dir = "attachments/SHOT/attempt-0";
  assert.deepEqual(asked, [
    ["app", `${dir}/app-1.png`],
    ["app", `${dir}/app-2.png`, "aside-1"],
    ["device", `${dir}/device-3.png`],
  ]);
  assert.deepEqual(shots[0], { path: `${dir}/app-1.png`, width: 2560, height: 1600, scale: 2 });
  assert.deepEqual(shots[2].scale, 2.75);
  assert.deepEqual(record.attachments.map((a) => [a.name, a.path, a.mimeType]),
    [["app-1.png", `${dir}/app-1.png`, "image/png"], ["app-2.png", `${dir}/app-2.png`, "image/png"]]);
  assert.equal(record.steps.find((s) => s.name === "device").attachments[0].path, `${dir}/device-3.png`);
  assert.ok(!attachments.has(`${dir}/app-1.png`), "the image never crosses the host as an artifact");
  const html = Buffer.from(attachments.get("report.html").base64, "base64").toString("utf8");
  assert.ok(html.includes(`src="${dir}/app-1.png"`), "the report shows the file next to it");
});

test("an unavailable capture rejects with CaptureUnavailableError, which a spec turns into a skip", async () => {
  const { attachments } = installFakeHost(createWorld());
  globalThis.__LINGXIA_AUTOMATION_HOST__.deviceScreenshot = async () => {
    throw coded("E_CAPTURE_PERMISSION", "grant Screen Recording to Ghostty in System Settings › Privacy & Security");
  };
  let caught;
  spec("on macos", { id: "SHOT-MAC", forensics: false }, async (t) => {
    await t.device.screenshot().catch((error) => {
      caught = error;
      if (error instanceof CaptureUnavailableError) t.skip(error.hint);
      throw error;
    });
  });

  await globalThis.__LINGXIA_TEST__.run();
  const record = report(attachments).cases[0];
  assert.ok(caught instanceof CaptureUnavailableError);
  assert.equal(caught.reason, "permission");
  assert.equal(caught.code, "E_CAPTURE_PERMISSION");
  assert.equal(caught.cause.code, "E_CAPTURE_PERMISSION");
  assert.equal(record.status, "skipped");
  assert.match(record.reason, /Screen Recording to Ghostty/);
  assert.deepEqual(record.attachments, []);
});

test("a host without captures is unsupported; a failed capture fails the spec", async () => {
  const { attachments } = installFakeHost(createWorld());
  let caught;
  spec("no capture", { id: "SHOT-NONE", forensics: false }, async (t) => {
    caught = await t.app.screenshot().catch((error) => error);
  });
  await globalThis.__LINGXIA_TEST__.run();
  assert.ok(caught instanceof CaptureUnavailableError);
  assert.equal(caught.reason, "unsupported");
  assert.match(caught.hint, /lingxia dev/);
  assert.equal(report(attachments).cases[0].status, "passed");

  reset();
  const failing = installFakeHost(createWorld());
  globalThis.__LINGXIA_AUTOMATION_HOST__.appScreenshot = async () => {
    throw coded("E_CAPTURE_FAILED", "no window to capture");
  };
  spec("failed", { id: "SHOT-FAILED", forensics: false }, (t) => t.app.screenshot());
  await globalThis.__LINGXIA_TEST__.run();
  const record = report(failing.attachments).cases[0];
  assert.equal(record.status, "failed");
  assert.equal(record.error.code, "E_CAPTURE_FAILED");
});

test("a surface must be a surface id", async () => {
  installFakeHost(createWorld());
  globalThis.__LINGXIA_AUTOMATION_HOST__.appScreenshot = async (name) => ({ path: name, width: 1, height: 1, scale: 1 });
  let caught;
  spec("bad surface", { id: "SHOT-SURFACE", forensics: false }, async (t) => {
    try {
      await t.app.screenshot({ surface: "" });
    } catch (error) {
      caught = error;
    }
  });
  await globalThis.__LINGXIA_TEST__.run();
  assert.ok(caught instanceof TypeError);
});
