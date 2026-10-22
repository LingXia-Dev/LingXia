#!/usr/bin/env node
// Enforce docs/internal/crate-layout.md: a workspace crate under `crates/`
// depends only on crates in its own layer or below.
//
//   cargo metadata --format-version 1 --no-deps | node scripts/ci/check-crate-layers.mjs
//
// A new crate must be given a layer here (and a line in the doc).

import process from "node:process";
import { pathToFileURL } from "node:url";

/** Layer per crate, `lingxia-` prefix omitted. Keep in step with the doc. */
export const LAYERS = {
  "app-context": 1,
  provider: 1,
  log: 1,
  messaging: 1,
  settings: 1,
  surface: 1,
  shell: 1,
  update: 1,
  "control-protocol": 1,
  webview: 2,
  platform: 2,
  "windows-contract": 2,
  "device-io": 2,
  lxapp: 3,
  logic: 3,
  service: 3,
  transfer: 3,
  media: 3,
  browser: 3,
  terminal: 3,
  "terminal-config": 3,
  "browser-shell": 4,
  proxy: 4,
  "control-commands": 4,
  "network-dev": 4,
  automation: 4,
  lingxia: 5,
  "control-runtime": 6,
  "windows-sdk": 6,
};

/** Build tooling: outside the layers, depended on from anywhere. */
export const TOOLING = new Set(["native-macros", "native-codegen", "windows-build"]);

function shortName(name) {
  if (name === "lingxia") return name;
  if (name === "lingxia-lxapp") return "lxapp";
  return name.replace(/^lingxia-/, "");
}

/** Problems with `metadata`'s `crates/` packages, as messages. */
export function checkLayers(metadata) {
  const problems = [];
  const crates = metadata.packages.filter((pkg) =>
    pkg.manifest_path.replaceAll("\\", "/").includes("/crates/"),
  );
  for (const pkg of crates) {
    const name = shortName(pkg.name);
    if (TOOLING.has(name)) continue;
    const layer = LAYERS[name];
    if (layer === undefined) {
      problems.push(`${pkg.name} has no layer: add it to scripts/ci/check-crate-layers.mjs and docs/internal/crate-layout.md`);
      continue;
    }
    for (const dep of pkg.dependencies) {
      if (dep.kind !== null) continue;
      const target = shortName(dep.name);
      const targetLayer = LAYERS[target];
      if (targetLayer === undefined || TOOLING.has(target)) continue;
      if (targetLayer > layer) {
        problems.push(`${pkg.name} (layer ${layer}) depends on ${dep.name} (layer ${targetLayer}), a layer above it`);
      }
    }
  }
  return problems;
}

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  return Buffer.concat(chunks).toString("utf8");
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const problems = checkLayers(JSON.parse(await readStdin()));
  for (const problem of problems) console.error(problem);
  if (problems.length > 0) process.exit(1);
}
