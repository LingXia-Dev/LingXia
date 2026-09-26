import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { registerOtherFileSpec } from "./helpers/other-file.mjs";
import { list, spec, reset, run } from "../dist/index.js";

const here = fileURLToPath(import.meta.url);

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

/** Registers three specs; returns the line of each `spec(` call. */
function registerThree(ran) {
  const line = () => Number(new Error().stack.split("\n")[2].match(/:(\d+):\d+\)?$/)[1]);
  const lines = [];
  lines.push(line()); spec("first", async () => { ran.push("first"); });
  lines.push(line()); spec("second", async () => { ran.push("second"); });
  lines.push(line()); spec("third", async () => { ran.push("third"); });
  return lines;
}

test("locations select the specs registered in their line ranges", async () => {
  const ran = [];
  const lines = registerThree(ran);
  registerOtherFileSpec("elsewhere");
  const locations = { [here]: [[lines[1], lines[1]], [lines[2], lines[2] + 1]] };
  installFakeHost(createWorld(), { control: { locations: JSON.stringify(locations) } });

  const report = await run();
  assert.deepEqual(ran, ["second", "third"]);
  assert.equal(report.filtered, true);
});

test("a file mapped to null selects all its specs", async () => {
  const ran = [];
  registerThree(ran);
  registerOtherFileSpec("elsewhere");
  installFakeHost(createWorld(), { control: { locations: JSON.stringify({ [here]: null }) } });

  const report = await run();
  assert.deepEqual(ran, ["first", "second", "third"]);
  assert.deepEqual(report.cases.map((c) => c.title), ["first", "second", "third"]);
});

test("list returns the selection and runs nothing", async () => {
  const ran = [];
  const lines = registerThree(ran);
  spec.configure({ tags: ["unit"] });
  installFakeHost(createWorld(), { control: { grep: "^(first|third)$" } });

  const report = await list();
  assert.deepEqual(ran, []);
  assert.equal(report.total, 0);
  assert.deepEqual(report.listed, [
    { id: "first", title: "first", file: here, line: lines[0], tags: ["unit"] },
    { id: "third", title: "third", file: here, line: lines[2], tags: ["unit"] },
  ]);
});

test("an empty listing is not an error", async () => {
  registerThree([]);
  installFakeHost(createWorld(), { control: { grep: "nothing" } });
  assert.deepEqual((await list()).listed, []);
});
