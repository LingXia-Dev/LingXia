// Print the package-lock entries reachable from the given workspace dirs, one
// sorted line each, so a lock edit for an unrelated workspace hashes the same.
// Usage: npm-lock-closure.mjs <workspace-dir>... < package-lock.json
import { readFileSync } from "node:fs";

const lock = JSON.parse(readFileSync(0, "utf8"));
const packages = lock.packages;
if (!packages) {
  // lockfileVersion 1 has no location map; fall back to the whole lock.
  process.stdout.write(`${JSON.stringify(lock)}\n`);
  process.exit(0);
}

const depFields = ["dependencies", "devDependencies", "optionalDependencies", "peerDependencies"];

// npm's lookup: the nearest node_modules walking up from the requiring location.
function resolve(from, name) {
  let dir = from;
  for (;;) {
    const candidate = dir ? `${dir}/node_modules/${name}` : `node_modules/${name}`;
    if (packages[candidate]) return candidate;
    if (!dir) return null;
    const cut = dir.lastIndexOf("/node_modules/");
    dir = cut >= 0 ? dir.slice(0, cut) : "";
  }
}

const seen = new Set();
const queue = process.argv.slice(2);
while (queue.length) {
  const location = queue.pop();
  if (seen.has(location) || !packages[location]) continue;
  seen.add(location);
  const entry = packages[location];
  if (entry.link && entry.resolved) queue.push(entry.resolved);
  for (const field of depFields) {
    for (const name of Object.keys(entry[field] ?? {})) {
      const target = resolve(location, name);
      if (target) queue.push(target);
    }
  }
}

const lines = [...seen]
  .sort((a, b) => (a < b ? -1 : a > b ? 1 : 0))
  .map((location) => `${location}\t${JSON.stringify(packages[location])}`);
process.stdout.write(lines.map((line) => `${line}\n`).join(""));
