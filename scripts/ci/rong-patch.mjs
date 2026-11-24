// The root Cargo.toml's rong patch block is the one source; every nested workspace that resolves
// rong carries an identical copy, since cargo reads [patch] only from the workspace root it builds.
// `node scripts/ci/rong-patch.mjs` fails on drift; `--write` copies the block.
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const START = "# rong master at one commit";
// Nested workspaces whose graph includes rong (through `lingxia` or `lingxia-view`).
export const PATCHED = [
  "crates/lingxia-view/fuzz/Cargo.toml",
  "crates/lingxia/tests/fixtures/authority-escape/Cargo.toml",
  "examples/lingxia-showcase/native/Cargo.toml",
  "examples/lingxia-showcase/windows/Cargo.toml",
];

/** The block: its comment lines and the `rong… = { git … }` lines after them. */
export function block(manifest) {
  const lines = manifest.split("\n");
  const start = lines.findIndex(line => line.startsWith(START));
  if (start < 0) return null;
  let end = start;
  while (end < lines.length && lines[end].startsWith("#")) end++;
  while (end < lines.length && /^rong\w* = /.test(lines[end])) end++;
  return { start, end, text: lines.slice(start, end).join("\n") };
}

/** `manifest` with `text` as the first entries of its [patch.crates-io] table. */
export function withBlock(manifest, text) {
  const current = block(manifest);
  let lines = manifest.split("\n");
  if (current) lines.splice(current.start, current.end - current.start);
  const header = lines.indexOf("[patch.crates-io]");
  if (header < 0) {
    while (lines.length && lines.at(-1) === "") lines.pop();
    return [...lines, "", "[patch.crates-io]", text, ""].join("\n");
  }
  lines.splice(header + 1, 0, text);
  return lines.join("\n");
}

function main() {
  const source = block(readFileSync(join(repo, "Cargo.toml"), "utf8"));
  if (!source) throw Error(`Cargo.toml: no rong patch block starting "${START}"`);
  const write = process.argv.includes("--write");
  const nested = execFileSync("git", ["ls-files", "*Cargo.toml"], { cwd: repo, encoding: "utf8" })
    .split("\n")
    .filter(path => path && path !== "Cargo.toml" && /^\[workspace\]/m.test(readFileSync(join(repo, path), "utf8")));
  const problems = nested
    .filter(path => !PATCHED.includes(path))
    .map(path => `${path}: a nested workspace not in PATCHED; add it if its graph includes rong`);
  for (const path of PATCHED) {
    const file = join(repo, path);
    const manifest = readFileSync(file, "utf8");
    const expected = withBlock(manifest, source.text);
    if (manifest === expected) continue;
    if (write) writeFileSync(file, expected);
    else problems.push(`${path}: its rong patch differs from Cargo.toml's; run node scripts/ci/rong-patch.mjs --write`);
  }
  if (problems.length) {
    console.error(problems.join("\n"));
    process.exit(1);
  }
  console.log(`rong patch: ${PATCHED.length} nested workspaces match Cargo.toml`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
