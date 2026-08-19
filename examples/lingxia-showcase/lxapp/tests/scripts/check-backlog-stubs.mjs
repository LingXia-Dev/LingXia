import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import stubs from "../pending/backlog-stubs.mjs";
import manifest from "../logic-api-coverage.mjs";

const pendingTest = path.resolve(import.meta.dirname, "../pending/backlog-pending.test.ts");

const allowedModes = new Set(["planned", "external-fixture", "external-ui"]);
const errors = [];

const ids = new Set();
for (const stub of stubs) {
  if (!stub.id || !/^PEND-[A-Z0-9-]+$/.test(stub.id)) {
    errors.push(`invalid stub id: ${JSON.stringify(stub.id)}`);
  }
  if (ids.has(stub.id)) errors.push(`duplicate stub id ${stub.id}`);
  ids.add(stub.id);
  if (!allowedModes.has(stub.mode)) {
    errors.push(`${stub.id} mode ${stub.mode} must stay off automated`);
  }
  if (stub.mode === "automated") {
    errors.push(`${stub.id} must not be automated`);
  }
  if (!stub.reason || stub.reason.trim().length < 8) {
    errors.push(`${stub.id} needs a reason`);
  }
  if (!Array.isArray(stub.covers) || stub.covers.length === 0) {
    errors.push(`${stub.id} needs covers`);
  }
}

const pendingSource = fs.readFileSync(pendingTest, "utf8");
if (!pendingSource.includes("backlog-stubs.mjs")) {
  errors.push("backlog-pending.test.ts must import backlog-stubs.mjs");
}
if (!pendingSource.includes("spec.skip")) {
  errors.push("backlog-pending.test.ts must register spec.skip stubs");
}

for (const entry of manifest.apis) {
  if (entry.mode !== "external-fixture" && entry.mode !== "external-ui") continue;
  if (!ids.has(entry.owner)) {
    errors.push(`${entry.api} ${entry.mode} owner ${entry.owner} is not a PEND stub id`);
  }
  const stub = stubs.find((item) => item.id === entry.owner);
  if (stub && !stub.covers.includes(entry.api)) {
    errors.push(`${entry.api} owner ${entry.owner} does not list that api in covers`);
  }
}

// backlog-stubs.mjs is the source of truth for pending holes. An optional
// planning doc may cross-reference the same ids; check it only when present.
const backlogPath = process.env.LX_COVERAGE_BACKLOG
  ? path.resolve(process.env.LX_COVERAGE_BACKLOG)
  : null;
if (backlogPath) {
  if (!fs.existsSync(backlogPath)) {
    errors.push(`LX_COVERAGE_BACKLOG points at a missing file: ${backlogPath}`);
  } else {
    const backlog = fs.readFileSync(backlogPath, "utf8");
    const classified = backlog.split(/\r?\n/).filter((line) =>
      /classified `(planned|external-fixture|external-ui)`/.test(line)
    );
    if (classified.length === 0) {
      errors.push("backlog has no classified planned/external-fixture/external-ui rows");
    }
    for (const line of classified) {
      const match = line.match(/PEND-[A-Z0-9-]+/);
      if (!match) {
        errors.push(`classified row missing PEND id: ${line.trim()}`);
        continue;
      }
      if (!ids.has(match[0])) {
        errors.push(`classified row names unknown stub ${match[0]}: ${line.trim()}`);
      }
    }
  }
}

if (errors.length > 0) {
  process.stderr.write(`${errors.map((item) => `- ${item}`).join("\n")}\n`);
  process.exit(1);
}

const counts = Object.fromEntries(
  [...allowedModes].map((mode) => [mode, stubs.filter((item) => item.mode === mode).length]),
);
process.stdout.write(
  `backlog stubs: ${stubs.length} registered ${JSON.stringify(counts)} ids=${[...ids].sort().join(",")}\n`,
);
