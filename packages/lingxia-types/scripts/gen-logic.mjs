import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const VERSION = "0.5.0";
const packageDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const workspaceDir = resolve(packageDir, "../..");
const installRoot = join(workspaceDir, "target", "rong-typegen", VERSION);
const binary = join(installRoot, "bin", process.platform === "win32" ? "rong-typegen.exe" : "rong-typegen");

const outputs = [
  join(packageDir, "src", "generated", "logic.ts"),
  join(packageDir, "src", "generated", "logic-web.d.ts"),
];
// rong-typegen 0.5 hard-codes a regeneration hint naming a package this
// workspace does not have; rewrite it to the real command.
const UPSTREAM_HINT = "// Do not edit by hand — run `cargo run -p rong_typegen` to regenerate.";
const REGEN_HINT = "// Do not edit by hand — run `npm run gen:logic` in packages/lingxia-types to regenerate.";

function run(command, args) {
  const result = spawnSync(command, args, { cwd: workspaceDir, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

if (!existsSync(binary)) {
  run("cargo", ["install", "rong_typegen", "--version", `=${VERSION}`, "--locked", "--root", installRoot]);
}

// --check is implemented here rather than passed to the binary: the committed
// outputs carry the rewritten hint, which the binary's internal comparison
// would always flag as drift.
const check = process.argv.slice(2).includes("--check");
const before = outputs.map((path) => (existsSync(path) ? readFileSync(path, "utf8") : null));

try {
  run(binary, ["--config", join(packageDir, "typegen.json")]);
  for (const path of outputs) {
    writeFileSync(path, readFileSync(path, "utf8").replace(UPSTREAM_HINT, REGEN_HINT));
  }
} catch (error) {
  outputs.forEach((path, i) => {
    if (before[i] !== null) writeFileSync(path, before[i]);
  });
  throw error;
}

if (check) {
  const stale = outputs.filter((path, i) => readFileSync(path, "utf8") !== before[i]);
  outputs.forEach((path, i) => {
    if (before[i] !== null) writeFileSync(path, before[i]);
  });
  if (stale.length > 0) {
    console.error(
      `generated types are out of date; run \`npm run gen:logic\`:${stale.map((path) => `\n  ${path}`).join("")}`,
    );
    process.exit(1);
  }
  for (const path of outputs) {
    console.log(`${path} is up to date.`);
  }
}
