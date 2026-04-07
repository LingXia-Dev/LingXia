import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";
import { minify } from "terser";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const packageDir = path.resolve(__dirname, "..");
const distDir = path.join(packageDir, "dist");
const tempFile = path.join(distDir, "runtime.es5.bundle.js");
const transpiledFile = path.join(distDir, "runtime.es5.transpiled.js");
const outputFile = path.join(distDir, "runtime.es5.js");

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
      "npx",
      ["rolldown", "-c"],
      {
        cwd: packageDir,
        stdio: "inherit",
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
      "npx",
      [
        "tsc",
        "--allowJs",
        "--target",
        "ES5",
        "--module",
        "none",
        "--outFile",
        path.basename(transpiledFile),
        path.basename(tempFile),
      ],
      {
        cwd: distDir,
        stdio: "inherit",
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
