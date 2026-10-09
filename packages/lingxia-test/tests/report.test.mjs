import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, expect } from "../dist/index.js";
import { reset, trackPublicSurface } from "../dist/runner.js";
import { looksSecretKey } from "../dist/redact.js";

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

test("a late equality difference survives truncated previews", async () => {
  const { attachments } = installFakeHost(createWorld());
  spec("compares the full value", { id: "DIFF-LATE", forensics: false }, () => {
    expect(`${"x".repeat(160)}a`).toEqual(`${"x".repeat(160)}b`);
  });
  await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  const assertion = report.cases[0].assertions[0];
  assert.equal(assertion.expected, assertion.actual, "preview truncation hides the unequal suffix");
  assert.match(assertion.difference, /UTF-16 offset 160/);
  assert.match(report.cases[0].error.difference, /UTF-16 offset 160/);
  assert.match(decodeAttachment(attachments, "report.html"), /UTF-16 offset 160/);
});

test("report timeline preserves step, assertion and attachment order", async () => {
  const { attachments } = installFakeHost(createWorld());
  spec("records the process", { id: "TRACE-ORDER", forensics: false }, async (t) => {
    await t.step("prepare", async () => {
      expect(1).toBe(1);
      await t.attach("input.txt", "order-42");
    });
  });
  await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  const step = report.cases[0].steps[0];
  assert.ok(step.sequence < step.assertions[0].sequence);
  assert.ok(step.assertions[0].sequence < step.attachments[0].sequence);
  const html = decodeAttachment(attachments, "report.html");
  assert.match(html, /Timeline &middot; 3 recorded events/);
});

test("passing attempt reports bounded network provenance and links timeline attachments", async (context) => {
  let now = Date.now();
  let advanceClock = true;
  // Cross a clock tick during step setup, then finish within the body's tick.
  context.mock.method(Date, "now", () => advanceClock ? now++ : now);
  let networkStart;
  const { attachments } = installFakeHost(createWorld(), {
    networkLog: () => [{ time: networkStart, kind: "fetch", method: "POST", url: "https://api.example/orders",
      status: 201, durationMs: 12, source: "route", answeredBy: "rule 1 (orders:ready)" }],
  });
  spec("creates an order", { id: "TRACE-NETWORK", forensics: false }, async (t) => {
    await t.step("create", async () => {
      advanceClock = false;
      networkStart = Date.now();
      await t.attach("order.json", { id: 42 });
    });
  });
  await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  const attempt = report.cases[0];
  assert.equal(attempt.status, "passed");
  assert.equal(attempt.network_calls[0].answeredBy, "rule 1 (orders:ready)");
  assert.equal(attempt.network_calls[0].observed_during, "create");
  assert.equal(attempt.steps[0].attachments[0].step, "create");
  assert.ok(attempt.network_calls[0].at_ms >= 0);
  const html = decodeAttachment(attachments, "report.html");
  assert.match(html, /network <code>POST https:\/\/api\.example\/orders<\/code> &middot; 201 &middot; rule 1 \(orders:ready\)/);
  assert.deepEqual(attempt.network_summary, { observed: 1, routed: 1, real: 0, limit_reached: false });
  assert.match(html, /attached <a href="attachments\/TRACE-NETWORK\/attempt-0\/order\.json">order\.json<\/a>/);
  assert.match(html, /1 network call started during this step/);
  assert.match(html, /Time-window observation; this step is not proven to have caused the call/);
});

test("the network summary counts every call; only the latest 20 are listed", async () => {
  const calls = (n) => Array.from({ length: n }, (_, i) => ({ time: Date.now(), kind: "fetch", method: "GET",
    url: `https://api.example/${i}`, status: 200, durationMs: 1, source: i % 5 === 0 ? "route" : "network" }));
  let count = 20;
  const { attachments } = installFakeHost(createWorld(), { networkLog: (_since, limit) => calls(count).slice(-limit) });
  spec("exactly twenty", { forensics: false }, () => {});
  spec("twenty-five", { forensics: false }, () => { count = 25; });
  await globalThis.__LINGXIA_TEST__.run();
  const [twenty, more] = JSON.parse(decodeAttachment(attachments, "report.json")).cases;
  assert.deepEqual(twenty.network_summary, { observed: 20, routed: 4, real: 16, limit_reached: false });
  assert.deepEqual(more.network_summary, { observed: 25, routed: 5, real: 20, limit_reached: true });
  assert.equal(more.network_calls.length, 20);
});

test("scoped nesting after await keeps its parent and overlapping top-level steps fail closed", async () => {
  const { attachments } = installFakeHost(createWorld());
  spec("scoped nesting", { id: "STEP-NEST", forensics: false }, async (t) => {
    await t.step("outer", async (step) => {
      await Promise.resolve();
      await step.step("inner", async () => { expect(1).toBe(1); });
    });
  });
  spec("ambiguous overlap", { id: "STEP-OVERLAP", forensics: false }, async (t) => {
    const first = t.step("A", async () => { await Promise.resolve(); expect(2).toBe(2); });
    const second = t.step("B", async () => {});
    await Promise.allSettled([first, second]);
  });
  spec("ambiguous sibling overlap", { id: "STEP-SIBLING", forensics: false }, async (t) => {
    await t.step("outer", async (step) => {
      const first = step.step("A", async () => { await Promise.resolve(); expect(3).toBe(3); });
      const second = step.step("B", async () => {});
      await Promise.allSettled([first, second]);
    });
  });
  await globalThis.__LINGXIA_TEST__.run();
  const [nested, overlap, sibling] = JSON.parse(decodeAttachment(attachments, "report.json")).cases;
  assert.equal(nested.status, "passed");
  assert.equal(nested.steps[0].steps[0].name, "inner");
  assert.equal(overlap.status, "failed", "catching the rejected step must not make the spec pass");
  assert.match(overlap.error.message, /Overlapping t\.step/);
  assert.deepEqual(overlap.steps.map((step) => step.name), ["A"]);
  assert.equal(sibling.status, "failed");
  assert.deepEqual(sibling.steps[0].steps.map((step) => step.name), ["A"]);
});

test("a caught step failure is the body's to handle; an unawaited child still fails the case", async () => {
  const { attachments } = installFakeHost(createWorld());
  spec("caught top-level", { id: "STEP-CAUGHT-TOP", forensics: false }, async (t) => {
    await t.step("top", async () => { throw new Error("top failed"); }).catch(() => {});
  });
  spec("caught child", { id: "STEP-CAUGHT", forensics: false }, async (t) => {
    await t.step("outer", async (step) => {
      await step.step("child", async () => { throw new Error("child failed"); }).catch(() => {});
    });
  });
  spec("unawaited child", { id: "STEP-UNAWAITED", forensics: false }, async (t) => {
    await t.step("outer", async (step) => {
      void step.step("child", async () => {
        await new Promise((resolve) => setTimeout(resolve, 20));
      }).catch(() => {});
    });
  });
  await globalThis.__LINGXIA_TEST__.run();
  const cases = JSON.parse(decodeAttachment(attachments, "report.json")).cases;
  assert.equal(cases[0].status, "passed");
  assert.equal(cases[0].steps[0].status, "failed", "the step row keeps its failure");
  assert.equal(cases[1].status, "passed");
  assert.equal(cases[2].status, "failed");
  assert.match(cases[2].error.message, /without awaiting|still running/i);
});

test("requested network recording that cannot start fails the case through normal cleanup", async () => {
  const { attachments, attempts } = installFakeHost(createWorld(), {
    control: { recordNetwork: "1" }, attempts: true,
  });
  const commands = [];
  globalThis.__LINGXIA_AUTOMATION_HOST__.networkRecord = (command) => {
    commands.push(command);
    if (command === "start") throw new Error("recorder unavailable");
    return { name: "discarded partial recording" };
  };
  spec("needs a recording", { id: "RECORD-SETUP", forensics: false }, async () => {
    throw new Error("body must not run");
  });
  spec("the run goes on", { forensics: false }, () => {});
  await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.equal(report.cases[0].status, "failed");
  assert.match(report.cases[0].error.message, /network recording could not start/);
  assert.equal(report.cases[1].status, "failed", "each case fails on its own setup");
  assert.deepEqual(commands, ["start", "stop", "start", "stop"]);
  assert.equal(attempts.open, undefined, "attempt cleanup still runs");
});

test("spec.fail treats a body rejection as the declared failure", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);
  class ApiError extends Error {}
  spec.fail("product rejects", async () => {
    throw new ApiError("upstream 500");
  });
  spec.fail("rejected promise", () => Promise.reject(new Error("nope")));
  spec.beforeEach(() => {});

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.deepEqual(report.cases.map((c) => c.status), ["xfail", "xfail"]);
  assert.equal(report.cases[0].error.message, "upstream 500");
  assert.equal(protocol.failed, 0);
});

test("a step failure the body observed never overrides spec.fail or a caught rejection", async () => {
  const { attachments } = installFakeHost(createWorld());
  spec.fail("fails in a step", { forensics: false }, async (t) => {
    await t.step("submit", () => { throw new Error("known bug"); });
  });
  spec("rejection pinned", { forensics: false }, async (t) => {
    await t.reject(() => t.step("submit", () => { throw new Error("refused"); }), { message: "refused" });
  });
  spec("rejection caught", { forensics: false }, async (t) => {
    try { await t.step("submit", () => { throw new Error("refused"); }); } catch { /* expected */ }
  });
  await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.deepEqual(report.cases.map((c) => c.status), ["xfail", "passed", "passed"], JSON.stringify(report.cases.map((c) => c.error)));
  assert.equal(report.cases[1].steps.find((step) => step.name === "submit")?.status ?? report.cases[1].steps[0].steps[0].status, "failed");
});

test("t.skip stops the spec and reports it skipped with its reason", async () => {
  const world = createWorld();
  const { attachments, events } = installFakeHost(world);
  let after = false, cleaned = false;
  spec("no offline client", { forensics: false }, async (t) => {
    t.defer(() => { cleaned = true; });
    t.skip("this account has no offline client");
    after = true;
  });
  spec("skip inside a step", { forensics: false }, async (t) => {
    await t.step("find client", () => t.skip("none found"));
  });
  spec.fail("skip wins over spec.fail", async (t) => {
    t.skip("not applicable");
  });
  spec("swallowed skip is still a skip", async (t) => {
    try { t.skip("caught"); } catch { /* the author swallowed it */ }
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.equal(after, false);
  assert.equal(cleaned, true);
  assert.deepEqual(report.cases.map((c) => c.status), ["skipped", "skipped", "skipped", "skipped"]);
  assert.deepEqual(report.cases.map((c) => c.reason),
    ["this account has no offline client", "none found", "not applicable", "caught"]);
  assert.equal(report.cases[0].error, undefined);
  assert.equal(report.cases[1].steps[0].status, "skipped");
  assert.equal(report.cases[0].attachments.length, 0, "a skip collects no failure forensics");
  assert.equal(protocol.skipped, 4);
  assert.equal(protocol.failed, 0);
  const finished = events.filter((e) => e.type === "case_finished");
  assert.equal(finished[0].status, "skipped");
  assert.match(decodeAttachment(attachments, "report.html"), /this account has no offline client/);
  assert.match(decodeAttachment(attachments, "junit.xml"), /<skipped message="none found"\/>/);
});

test("declared secret args are masked everywhere but reach the spec", async () => {
  const world = createWorld();
  const { attachments, events } = installFakeHost(world, {
    args: { password: "hunter22", pin: "4711-9", user: "alice" },
    control: { secretArgs: JSON.stringify(["password", "pin"]) },
    logs: "login with hunter22 ok",
  });
  let seen;
  spec("logs in", async (t) => {
    seen = { password: t.arg("password"), pin: t.arg("pin") };
    await t.attach("note.txt", `typed ${t.arg("password")}`);
    await t.attach("form.json", { pin: t.arg("pin"), nested: [`pw:${t.arg("password")}`] });
    await t.attach("raw.txt", { mimeType: "text/plain", base64: Buffer.from(`pin=${t.arg("pin")}`).toString("base64") });
    expect(`pw=${t.arg("password")} pin=${t.arg("pin")}`).toBe("x");
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(seen.password, "hunter22");
  assert.equal(seen.pin, "4711-9");
  const texts = [...attachments.keys()].map((name) => [name, decodeAttachment(attachments, name)]);
  assert.ok(texts.some(([name]) => name.endsWith("/logs.txt")), "forensics attached logs.txt");
  for (const [name, text] of texts) {
    for (const secret of ["hunter22", "4711-9"]) {
      assert.ok(!text.includes(secret), `${name} leaks ${secret}`);
    }
  }
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.equal(report.meta.args.password, "***");
  assert.equal(report.meta.args.pin, "***");
  assert.equal(report.meta.args.user, "alice");
  assert.match(report.cases[0].error.message, /pw=\*\*\* pin=\*\*\*/);
  assert.ok(!JSON.stringify(events).includes("hunter22"), "events leak the secret");
  assert.equal(events.find((e) => e.type === "run_started").args.password, "***");
  assert.equal(protocol.meta.args.password, "***");
});

test("a credential-named --arg is masked in meta.args only", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world, {
    args: { apiKey: "k-123456", maxTokens: "1000", passWithNoTests: "yes" },
    control: {},
  });
  spec("echoes", { forensics: false }, async (t) => {
    expect(`${t.arg("apiKey")}/${t.arg("maxTokens")}`).toBe("x");
  });
  await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.deepEqual(report.meta.args, { apiKey: "***", maxTokens: "1000", passWithNoTests: "yes" });
  // A guess from the name never blanks the value elsewhere.
  assert.match(report.cases[0].error.message, /k-123456\/1000/);
});

test("report.html stays well-formed when a secret overlaps markup", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world, {
    args: { token: "</script>" },
    control: { secretArgs: '["token"]' },
  });
  spec("echoes", { forensics: false }, async (t) => {
    expect(`x${t.arg("token")}`).toBe("y");
  });
  await globalThis.__LINGXIA_TEST__.run();
  const html = decodeAttachment(attachments, "report.html");
  assert.equal(html.match(/<\/script>/g)?.length, html.match(/<script\b/g)?.length);
  assert.doesNotMatch(JSON.parse(decodeAttachment(attachments, "report.json")).cases[0].error.message, /<\/script>/);
});

test("run controls stay out of the args and a user arg never filters the run", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world, {
    args: { id: "user-value", grep: "nothing-matches" },
    control: { id: "second", platform: "ios" },
  });
  let seen;
  spec("first", () => {});
  spec("second", (t) => { seen = { id: t.arg("id"), grep: t.arg("grep"), platform: t.arg("platform", { required: false }) }; });
  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.deepEqual(protocol.cases.map((c) => c.id), ["second"]);
  assert.deepEqual(seen, { id: "user-value", grep: "nothing-matches", platform: undefined });
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.deepEqual(report.meta.run, { id: "second", platform: "ios" });
  assert.equal(report.meta.platform, "ios");
  assert.equal(report.meta.args.id, "user-value");
});

test("credential names match whole trailing words, not substrings", () => {
  for (const key of ["password", "PASSWORD", "DB_PASSWORD", "userPasswd", "pwd", "clientSecret",
    "authToken", "refresh_token", "x-api-key", "apiKey", "APIKey", "API_KEY", "apikey",
    "credentials", "gcpCredential", "PRIVATE_KEY", "passphrase"]) {
    assert.equal(looksSecretKey(key), true, key);
  }
  for (const key of ["passWithNoTests", "bypassCache", "maxTokens", "tokenCount", "passport",
    "secretName", "user", "baseUrl", "keyboard", "pass", "compass", "tokenizer"]) {
    assert.equal(looksSecretKey(key), false, key);
  }
});

test("spec.fail with expected grades only the matching failure xfail", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);
  class ApiError extends Error {
    constructor(code, message) { super(message); this.code = code; }
  }
  spec.fail("known code", { expected: { code: "E_QUOTA" } }, async () => {
    throw new ApiError("E_QUOTA", "quota exceeded");
  });
  spec.fail("known message regex", { expected: { message: /quota/ } }, async () => {
    throw new Error("quota exceeded");
  });
  spec.fail("other failure", { expected: { code: "E_QUOTA", message: "quota" } }, async (t) => {
    await t.app.view.testId("mistyped").click({ timeout: 20, interval: 5 });
  });
  spec.fail("known but passes", { expected: { code: "E_QUOTA" } }, async () => {});

  await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.deepEqual(report.cases.map((c) => c.status), ["xfail", "xfail", "failed", "xpass"]);
  assert.match(report.cases[2].error.message,
    /spec\.fail expected a failure with code "E_QUOTA", got "E_TIMEOUT" and a message containing "quota"; the body failed differently/);
  assert.throws(() => spec("not fail", { expected: { code: "X" } }, () => {}), /only meaningful for spec\.fail/);
  assert.throws(() => spec.fail("empty", { expected: {} }, () => {}), /needs a code or a message/);
});

test("t.skip rejects during cleanup", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);
  spec("skip in defer", { forensics: false }, async (t) => {
    t.defer(() => t.skip("too late"));
  });
  spec.afterEach((t) => t.skip("also too late"));
  spec("skip in afterEach", { forensics: false }, async () => {});

  await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  for (const item of report.cases) {
    assert.equal(item.status, "failed");
    assert.equal(item.error.phase, "defer");
    assert.match(item.error.message, /t\.skip cannot be called during cleanup/);
  }
});

test("spec.fail grades a body assertion xfail, a pass xpass, and a timeout timeout", async () => {
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
    await t.app.logic.eval(() => 1);
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.equal(report.cases[0].status, "xfail");
  assert.equal(report.cases[1].status, "xpass");
  assert.equal(report.cases[2].status, "timeout");
  assert.equal(protocol.xfail, 1);
  assert.equal(protocol.xpass, 1);
  assert.equal(protocol.timeout, 1);
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
  assert.ok(paths.includes("attachments/fails-for-forensics/attempt-0/failure.png"));
  assert.equal(failed.attachments.find((item) => item.name === "failure.png").step, undefined);
  assert.equal(failed.attachments.find((item) => item.name === "failure.png").purpose, "failure_evidence");
  assert.ok(paths.includes("attachments/fails-for-forensics/attempt-0/forensics.json"));
  assert.ok(paths.includes("attachments/fails-for-forensics/attempt-0/logs.txt"));
  assert.ok(paths.includes("attachments/fails-for-forensics/attempt-0/note.txt"));
  assert.equal(report.partial, false);

  const html = decodeAttachment(attachments, "report.html");
  assert.match(html, /<!DOCTYPE html>/);
  assert.doesNotMatch(html, /cdn\.|unpkg|jsdelivr|https:\/\/fonts/);
  assert.match(html, /data:image\/png;base64,/);
  assert.match(html, /attachments\/fails-for-forensics\/attempt-0\/failure\.png/);
  assert.match(html, /failure\.png &middot; failure evidence for this attempt/);
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
    args: { framework: "react" },
    control: { platform: "windows" },
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
  assert.equal(report.meta.run.platform, "windows");
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

test("expect.poll(fn) retries an arbitrary read", async () => {
  const world = createWorld();
  installFakeHost(world);
  let value = 0;
  spec("polls", async (t) => {
    setTimeout(() => {
      value = 3;
    }, 30);
    await expect.poll(() => value, { timeout: 400 }).toBe(3);
  });
  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.passed, 1);
});

test("attaches a CI-ingestible junit.xml alongside the HTML report", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world, { control: { platform: "macos" } });

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
    await t.app.logic.eval(() => "probe");
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

test("returned-object tags are not claimed against lx.* call records", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);
  world.setCalls("probe", ["lx.getStorage", "lx.downloadFile", "lx.fs.file"]);

  // The recorder never emits `Storage.set` / `DownloadTask.wait` / `LxFile.text`.
  // Matching those strings against `calls` would paint every eval-driven
  // object cover as claimed, including LOGIC-003 and TRANSFER-DOWNLOAD-001.
  spec("uses returned objects", {
    id: "COV-OBJ",
    covers: ["lx.getStorage", "Storage.set", "DownloadTask.wait", "LxFile.text"],
  }, async (t) => {
    await t.app.logic.eval(() => "probe");
    expect(1).toBe(1);
  });

  await globalThis.__LINGXIA_TEST__.run();
  const html = decodeAttachment(attachments, "report.html");

  assert.match(html, /class="cover cover-ok"[^>]*>lx\.getStorage</);
  assert.match(html, /class="cover cover-ok"[^>]*>Storage\.set</);
  assert.match(html, /class="cover cover-ok"[^>]*>DownloadTask\.wait</);
  assert.match(html, /class="cover cover-ok"[^>]*>LxFile\.text</);
  assert.doesNotMatch(html, /class="cover cover-claimed"[^>]*>Storage\.set</);
  assert.doesNotMatch(html, /class="cover cover-claimed"[^>]*>DownloadTask\.wait</);
  assert.doesNotMatch(html, /class="cover cover-claimed"[^>]*>LxFile\.text</);
});

test("a parent lx.* path does not prove a primitive member", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);
  world.setCalls("probe", ["lx.env"]);

  spec("reads env", {
    id: "COV-ENV-PARENT",
    covers: ["lx.env", "lx.env.USER_DATA_PATH"],
  }, async (t) => {
    await t.app.logic.eval(() => "probe");
    expect(1).toBe(1);
  });

  await globalThis.__LINGXIA_TEST__.run();
  const html = decodeAttachment(attachments, "report.html");

  assert.match(html, /class="cover cover-ok"[^>]*>lx\.env</);
  assert.match(html, /class="cover cover-claimed"[^>]*>lx\.env\.USER_DATA_PATH</);
});

test("a primitive lx.* member path in calls is behaviour", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);
  world.setCalls("probe", ["lx.env", "lx.env.USER_DATA_PATH"]);

  spec("reads the sandbox root", {
    id: "COV-ENV-MEMBER",
    covers: ["lx.env", "lx.env.USER_DATA_PATH"],
  }, async (t) => {
    await t.app.logic.eval(() => "probe");
    expect(1).toBe(1);
  });

  await globalThis.__LINGXIA_TEST__.run();
  const html = decodeAttachment(attachments, "report.html");

  assert.match(html, /class="cover cover-ok"[^>]*>lx\.env</);
  assert.match(html, /class="cover cover-ok"[^>]*>lx\.env\.USER_DATA_PATH</);
  assert.doesNotMatch(html, /class="cover cover-claimed"[^>]*>lx\.env\.USER_DATA_PATH</);
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
    seen = await t.app.logic.eval(() => "returns undefined");
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
    appId: "acme-notes",
    appName: "Acme Notes",
    version: "2.1.0",
    releaseType: "developer",
    pagesCount: 4,
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
    await t.app.view.testId("home-name").fill("Ada");
    await t.app.logic.eval(() => 1 + 1);
  });

  await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  const trace = report.cases[0].steps;

  assert.deepEqual(
    trace.map((entry) => `${entry.kind} ${entry.name} ${entry.detail}`),
    ["action nav.relaunch home", 'action page.fill [data-testid="home-name"] value="Ada"', "action logic.eval () => 1 + 1"],
  );
  assert.ok(trace.every((entry) => entry.status === "passed"));

  const html = decodeAttachment(attachments, "report.html");
  assert.match(html, /nav\.relaunch/);
  assert.match(html, /logic\.eval/);
});

test("a retry loop records one action, not one per poll", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);
  let reads = 0;

  spec("polls", { id: "TRACE-2" }, async (t) => {
    await t.step("wait for the value", async () => {
      await expect.poll(async () => {
        reads += 1;
        await t.app.logic.eval(() => 1);
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
    // No expect.poll(fn) here — the shape a project's own helper takes.
    for (let attempt = 0; attempt < 6; attempt += 1) {
      await t.app.nav.current();
    }
    await t.app.logic.eval(() => 1);
  });

  await globalThis.__LINGXIA_TEST__.run();
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  const trace = report.cases[0].steps;

  assert.equal(trace.length, 2, JSON.stringify(trace.map((s) => s.name)));
  assert.equal(trace[0].name, "nav.current");
  assert.equal(trace[0].repeat, 6);
  assert.equal(trace[1].name, "logic.eval");
  assert.equal(trace[1].repeat, undefined);

  const html = decodeAttachment(attachments, "report.html");
  assert.match(html, /nav\.current<\/code>.*?&times;6/s);
});

test("an app name cannot inject markup into the report", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world, { control: { platform: "<b>win</b>" } });
  globalThis.lx.automation().lxapp().info = async () => ({
    appId: "evil",
    appName: 'Cats & Dogs <img src=x onerror=alert(1)>',
    version: "1.0.0",
    releaseType: "developer",
    pagesCount: 1,
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
    await t.app.logic.eval(() => 1);
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.cases[0].status, "timeout");

  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  const action = report.cases[0].steps[0];
  assert.equal(action.name, "logic.eval");
  // A hung call rendered as an instant success is the one thing the trace
  // exists to prevent.
  assert.equal(action.status, "timeout");
  assert.ok(action.error, "the abandoned action needs its error");
});


test("partial reports cannot appear green in JUnit even with no failed cases", async () => {
  const { renderJUnit } = await import("../dist/junit.js");
  const xml = renderJUnit({ partial: true, total: 0, passed: 0, failed: 0, timeout: 0,
    skipped: 0, xfail: 0, xpass: 0, duration_ms: 1, cases: [] });
  assert.match(xml, /tests="1" failures="0" errors="1"/);
  assert.match(xml, /<error message="The test run did not finish"/);
});

test("a failed toMatchSchema points at its own line, and a unit spec names no page", async () => {
  const API = {
    openapi: "3.1.0",
    info: { title: "t", version: "1" },
    paths: {},
    components: { schemas: { Slot: { type: "object", properties: { "at:10:20": { type: "string" } } } } },
  };
  const world = createWorld();
  world.app.network = { async calls() { return []; }, async captureResponses() { return true; } };
  installFakeHost(world, { control: { openapi: JSON.stringify([{ name: "slots.yaml", doc: API }]) } });
  const here = fileURLToPath(import.meta.url).replaceAll("\\", "/");
  let assertionLine;

  spec("unit schema", { tags: ["unit"] }, async () => {
    const slot = { "at:10:20": 1 };
    assertionLine = Number(new Error().stack.split("\n")[1].match(/:(\d+):\d+\)?$/)[1]) + 1;
    expect(slot).toMatchSchema("Slot");
  });
  spec("on a page", async (t) => {
    await t.app.nav.to({ page: "onboarding" });
    expect({ "at:10:20": 1 }).toMatchSchema("Slot");
  });

  const report = await globalThis.__LINGXIA_TEST__.run();
  const [unit, paged] = report.cases;
  // The issue line in the message looks like a frame; it is not one.
  assert.match(unit.error.message, /at \/at:10:20: expected string/);
  assert.equal(unit.error.location, `${here}:${assertionLine}:18`);
  assert.equal(unit.error.page, undefined, "a spec that touched no page is on none");
  assert.equal(paged.error.page?.name, "onboarding");
});
