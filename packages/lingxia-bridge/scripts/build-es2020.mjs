import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";
import { minify } from "terser";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const packageDir = path.resolve(__dirname, "..");
const distDir = path.join(packageDir, "dist");
const tempFile = path.join(distDir, "bridge-runtime.es2020.bundle.js");
const outputFile = path.join(distDir, "bridge-runtime.es2020.js");
const rolldownBin = path.join(
  packageDir,
  "node_modules",
  ".bin",
  process.platform === "win32" ? "rolldown.cmd" : "rolldown",
);

await fs.mkdir(distDir, { recursive: true });
await runRolldown();

try {
  const source = await fs.readFile(tempFile, "utf8");
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
