import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";
import { minify } from "terser";
import { resolveBin } from "./resolve-bin.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const packageDir = path.resolve(__dirname, "..");
const distDir = path.join(packageDir, "dist");
const tempFile = path.join(distDir, "bridge-runtime.es5.bundle.js");
const transpiledFile = path.join(distDir, "bridge-runtime.es5.transpiled.js");
const outputFile = path.join(distDir, "bridge-runtime.es5.js");
const rolldownBin = resolveBin(packageDir, "rolldown");
const tscBin = resolveBin(packageDir, "tsc");

await fs.mkdir(distDir, { recursive: true });
await runRolldown();
await runTsc();

try {
  const source = await fs.readFile(transpiledFile, "utf8");
  const result = await minify(source, {
    compress: true,
    mangle: true,
    format: {
      comments: false,
    },
  });
  if (!result.code) {
    throw new Error("terser did not produce any output");
  }
  await fs.writeFile(outputFile, result.code);
} finally {
  await fs.rm(tempFile, { force: true });
  await fs.rm(transpiledFile, { force: true });
}

function runRolldown() {
  return new Promise((resolve, reject) => {
    const child = spawn(
      rolldownBin,
      ["-c"],
      {
        cwd: packageDir,
        stdio: "inherit",
        // On Windows the .bin entry is rolldown.cmd; route through the shell so
        // it resolves (node's spawn can't exec a .cmd directly -> spawn EINVAL).
        shell: process.platform === "win32",
        env: {
          ...process.env,
          LX_RUNTIME_PLATFORM: "mobile",
          RUNTIME_OUTPUT: path.basename(tempFile),
        },
      },
    );

    child.on("exit", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`rolldown failed with exit code ${code ?? "unknown"}`));
      }
    });
    child.on("error", reject);
  });
}

function runTsc() {
  return new Promise((resolve, reject) => {
    const child = spawn(
      tscBin,
      [
        "--allowJs",
        "--target",
        "ES5",
        "--module",
        "none",
        // Transpiling a self-contained bundle: ignore any ambient @types that an
        // npm-workspace install may have hoisted into the root node_modules.
        "--skipLibCheck",
        "--outFile",
        path.basename(transpiledFile),
        path.basename(tempFile),
      ],
      {
        cwd: distDir,
        stdio: "inherit",
        // tsc on Windows is tsc.cmd; route through the shell (see above).
        shell: process.platform === "win32",
        env: process.env,
      },
    );

    child.on("exit", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`tsc failed with exit code ${code ?? "unknown"}`));
      }
    });
    child.on("error", reject);
  });
}
