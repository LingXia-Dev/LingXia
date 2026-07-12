import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const VERSION = "0.5.0";
const packageDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const workspaceDir = resolve(packageDir, "../..");
const installRoot = join(workspaceDir, "target", "rong-typegen", VERSION);
const binary = join(installRoot, "bin", process.platform === "win32" ? "rong-typegen.exe" : "rong-typegen");

function run(command, args) {
  const result = spawnSync(command, args, { cwd: workspaceDir, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

if (!existsSync(binary)) {
  run("cargo", ["install", "rong_typegen", "--version", `=${VERSION}`, "--locked", "--root", installRoot]);
}

run(binary, ["--config", join(packageDir, "typegen.json"), ...process.argv.slice(2)]);
