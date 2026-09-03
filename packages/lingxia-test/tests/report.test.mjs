import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, expect, reset, trackPublicSurface } from "../dist/index.js";

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

test("attaches a CI-ingestible junit.xml alongside the HTML report", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world, { args: { platform: "macos" } });

  spec("passes", { id: "JUNIT-OK", covers: ["lx.getStorage"] }, async () => {
    expect(1).toBe(1);
  });
  spec("breaks", { id: "JUNIT-BAD" }, async () => {
    expect("left").toBe("right");
  });
  spec.skip("pending", { id: "JUNIT-PEND", reason: "OS dialog" });

  await globalThis.__LINGXIA_TEST__.run();
  const xml = decodeAttachment(attachments, "junit.xml");

  assert.match(xml, /^<\?xml version="1\.0" encoding="UTF-8"\?>/);
  assert.match(xml, /<testsuites [^>]*tests="3"[^>]*failures="1"[^>]*skipped="1"/);
  assert.match(xml, /<testcase name="passes"/);
  assert.match(xml, /<failure message="[^"]*" type="AssertionError">/);
  assert.match(xml, /<skipped message="OS dialog"\/>/);
  assert.match(xml, /<property name="covers" value="lx.getStorage"\/>/);
  // A failure message spanning lines must not break the attribute.
  assert.doesNotMatch(xml, /message="[^"]*\n/);
});

test("an app that declares no coverage gets no coverage panel", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);

  // What `lingxia new` scaffolds: one journey spec, no cover tags.
  spec("home greets by name", async () => {
    expect(1).toBe(1);
  });

  await globalThis.__LINGXIA_TEST__.run();
  const html = decodeAttachment(attachments, "report.html");

  assert.doesNotMatch(html, /coverage/i);
  // A new app never claimed the rest of the platform; listing it would read
  // as failing to cover an API it has nothing to do with.
  assert.doesNotMatch(html, /lx\.vibrateShort/);
  assert.doesNotMatch(html, /Logic API/);
});

test("declared tags are the default coverage scope", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);

  spec("proves storage behaviour", { id: "COV-1", covers: ["lx.getStorage"] }, async () => {
    expect(1).toBe(1);
  });
  spec.skip("pending hole", { id: "COV-3", covers: ["lx.share"], reason: "OS share sheet" });

  await globalThis.__LINGXIA_TEST__.run();
  const html = decodeAttachment(attachments, "report.html");

  assert.match(html, /Capability coverage/);
  assert.match(html, /1\/2 declared capabilities proven/);
  assert.match(html, /class="cover cover-ok"[^>]*>lx\.getStorage</);
  assert.match(html, /class="cover cover-pending"[^>]*>lx\.share</);
  // Everything the suite never mentioned stays out of the report.
  assert.doesNotMatch(html, /lx\.vibrateShort/);
  assert.doesNotMatch(html, /lx API coverage/);
});

test("a passing spec that never reaches its tag is not credited with it", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);
  world.setCalls("probe", ["lx.getStorage"]);

  // Declares two capabilities and only ever touches one of them.
  spec("claims more than it does", {
    id: "COV-CLAIM",
    covers: ["lx.getStorage", "lx.tray"],
  }, async (t) => {
    await t.app.eval({ script: "probe" });
    expect(1).toBe(1);
  });

  await globalThis.__LINGXIA_TEST__.run();
  const html = decodeAttachment(attachments, "report.html");

  // Reached, so proven.
  assert.match(html, /class="cover cover-ok"[^>]*>lx\.getStorage</);
  // Declared by a passing spec, never called: the whole point is that this
  // does not read the same as the one above.
  assert.match(html, /class="cover cover-claimed"[^>]*>lx\.tray</);
  assert.match(html, /but no eval in those specs reached it/);
});

test("an eval whose script returns undefined yields undefined, not the envelope", async () => {
  const world = createWorld();
  installFakeHost(world);
  // The capture envelope carries no `value` key when the script returns
  // undefined, so recognising it by shape handed the envelope back as the
  // result and every `result == null` assertion downstream flipped.
  world.setEval("returns undefined", undefined);
  world.setCalls("returns undefined", ["lx.getStorage"]);

  let seen = "unset";
  let observed = null;
  spec("reads an undefined result", { id: "COV-UNDEF", covers: ["lx.getStorage"] }, async (t) => {
    seen = await t.app.eval({ script: "returns undefined" });
    observed = [...t.observed];
  });

  await globalThis.__LINGXIA_TEST__.run();
  assert.strictEqual(seen, undefined);
  // The calls still land, so the value fix does not cost the measurement.
  assert.deepStrictEqual(observed, ["lx.getStorage"]);
});

test("a spec that runs no eval keeps its declared coverage", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);

  // Driving through the page or native chrome is a legitimate way to exercise
  // an API, and it produces no eval to observe. Absence of observation must not
  // read as absence of coverage, or every DOM-driven spec regresses at once.
  spec("drives without eval", { id: "COV-NOEVAL", covers: ["lx.getStorage"] }, async () => {
    expect(1).toBe(1);
  });

  await globalThis.__LINGXIA_TEST__.run();
  const html = decodeAttachment(attachments, "report.html");
  assert.match(html, /class="cover cover-ok"[^>]*>lx\.getStorage</);
  // The legend and stylesheet always mention the class; what must not appear is
  // this capability wearing it.
  assert.doesNotMatch(html, /class="cover cover-claimed"[^>]*>lx\.getStorage</);
});

test("an expected failure is not coverage", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);

  // xfail records a known-broken outcome. Counting it credits the suite for
  // the one result it has already admitted does not work.
  spec.fail("known broken", { id: "COV-XFAIL", covers: ["lx.share"], reason: "upstream" }, async () => {
    expect(1).toBe(2);
  });

  await globalThis.__LINGXIA_TEST__.run();
  const html = decodeAttachment(attachments, "report.html");
  assert.doesNotMatch(html, /class="cover cover-ok"[^>]*>lx\.share</);
});

test("a conformance suite opts into the whole published surface", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);
  trackPublicSurface();

  spec("proves storage behaviour", { id: "COV-1", covers: ["lx.getStorage"] }, async () => {
    expect(1).toBe(1);
  });
  spec("only proves a member exists", { id: "COV-2", covers: ["shape:lx.getLocation"] }, async () => {
    expect(1).toBe(1);
  });
  spec.skip("pending hole", { id: "COV-3", covers: ["lx.share"], reason: "OS share sheet" });

  await globalThis.__LINGXIA_TEST__.run();
  const html = decodeAttachment(attachments, "report.html");

  assert.match(html, /lx API coverage/);
  assert.match(html, /Logic API \(lx\.\*\)/);
  // Now an untested capability is a hole worth showing.
  assert.match(html, /class="cover cover-none"[^>]*>lx\.vibrateShort</);
  assert.match(html, /class="cover cover-ok"[^>]*>lx\.getStorage</);
  assert.match(html, /class="cover cover-shape"[^>]*>lx\.getLocation</);
  assert.match(html, /class="cover cover-pending"[^>]*>lx\.share</);
});

test("the report is named after the app under test", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);
  globalThis.lx.automation().lxapp().info = async () => ({
    appid: "acme-notes",
    app_name: "Acme Notes",
    version: "2.1.0",
    release_type: "developer",
    pages_count: 4,
  });

  spec("passes", async () => {
    expect(1).toBe(1);
  });

  await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.deepEqual(report.meta.subject, {
    appid: "acme-notes",
    app_name: "Acme Notes",
    version: "2.1.0",
    release_type: "developer",
    pages: 4,
  });

  const html = decodeAttachment(attachments, "report.html");
  assert.match(html, /<title>Acme Notes test report<\/title>/);
  assert.match(html, /class="eyebrow">Acme Notes/);
  assert.doesNotMatch(html, /<title>lxdev test report<\/title>/);
});

test("an unreachable app still produces a titled report", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);
  globalThis.lx.automation().lxapp().info = async () => {
    throw new Error("app is not up");
  };

  spec("passes", async () => {
    expect(1).toBe(1);
  });

  await globalThis.__LINGXIA_TEST__.run();
  const html = decodeAttachment(attachments, "report.html");
  assert.match(html, /<title>lxapp test report<\/title>/);
});

test("a spec that never calls t.step still records what it did", async () => {
  const world = createWorld();
  world.add({ testId: "home-name", visible: true, value: "" });
  const { attachments } = installFakeHost(world);

  spec("flat spec", { id: "TRACE-1" }, async (t) => {
    await t.app.nav.relaunch({ page: "home" });
    await t.app.page.testId("home-name").fill("Ada");
    await t.app.eval({ script: "return 1 + 1;" });
  });

  await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  const trace = report.cases[0].steps;

  assert.deepEqual(
    trace.map((entry) => `${entry.kind} ${entry.name} ${entry.detail}`),
    ["action nav.relaunch home", 'action page.fill [data-testid="home-name"]', "action app.eval return 1 + 1;"],
  );
  assert.ok(trace.every((entry) => entry.status === "passed"));

  const html = decodeAttachment(attachments, "report.html");
  assert.match(html, /nav\.relaunch/);
  assert.match(html, /app\.eval/);
});

test("a retry loop records one action, not one per poll", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);
  let reads = 0;

  spec("polls", { id: "TRACE-2" }, async (t) => {
    await t.step("wait for the value", async () => {
      await t.expect.poll(async () => {
        reads += 1;
        await t.app.eval({ script: "return 1;" });
        return reads;
      }, { timeout: 800, interval: 20 }).toBe(5);
    });
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 0);
  assert.ok(reads >= 5, `expected several polls, got ${reads}`);

  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  const inner = report.cases[0].steps[0].steps;
  assert.deepEqual(inner, [], "polling must not emit one row per attempt");
});

test("a hand-rolled poll collapses into one row with a count", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);

  spec("polls by hand", { id: "TRACE-3" }, async (t) => {
    // No t.expect.poll here — the shape a project's own helper takes.
    for (let attempt = 0; attempt < 6; attempt += 1) {
      await t.app.nav.current();
    }
    await t.app.eval({ script: "return 1;" });
  });

  await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  const trace = report.cases[0].steps;

  assert.equal(trace.length, 2, JSON.stringify(trace.map((s) => s.name)));
  assert.equal(trace[0].name, "nav.current");
  assert.equal(trace[0].repeat, 6);
  assert.equal(trace[1].name, "app.eval");
  assert.equal(trace[1].repeat, undefined);

  const html = decodeAttachment(attachments, "report.html");
  assert.match(html, /nav\.current<\/code>.*?&times;6/s);
});

test("an app name cannot inject markup into the report", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world, { args: { platform: "<b>win</b>" } });
  globalThis.lx.automation().lxapp().info = async () => ({
    appid: "evil",
    app_name: 'Cats & Dogs <img src=x onerror=alert(1)>',
    version: "1.0.0",
    release_type: "developer",
    pages_count: 1,
  });

  spec("passes", async () => {
    expect(1).toBe(1);
  });

  await globalThis.__LINGXIA_TEST__.run();
  const html = decodeAttachment(attachments, "report.html");

  assert.doesNotMatch(html, /<img src=x/);
  assert.doesNotMatch(html, /<b>win<\/b>/);
  assert.match(html, /&lt;img src=x onerror=alert\(1\)&gt;/);
});

test("a spec timeout marks the action that never returned", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);
  const driver = globalThis.lx.automation().lxapp();
  driver.eval = () => new Promise(() => {});

  spec("hangs in a driver call", { id: "HANG-1", timeout: 120 }, async (t) => {
    await t.app.eval({ script: "return 1;" });
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.cases[0].status, "failed");

  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  const action = report.cases[0].steps[0];
  assert.equal(action.name, "app.eval");
  // A hung call rendered as an instant success is the one thing the trace
  // exists to prevent.
  assert.equal(action.status, "timeout");
  assert.ok(action.error, "the abandoned action needs its error");
});
