import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Copy the framework's source SFCs and the Vue shim into dist. Done in node
// (not `cp src/*.vue`) so it works on Windows, where cmd.exe does not expand the
// glob and `cp` is unavailable.
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const srcDir = path.resolve(__dirname, "..", "src");
const distDir = path.resolve(__dirname, "..", "dist");

fs.mkdirSync(distDir, { recursive: true });
for (const entry of fs.readdirSync(srcDir)) {
  if (entry.endsWith(".vue") || entry === "shims-vue.d.ts") {
    fs.copyFileSync(path.join(srcDir, entry), path.join(distDir, entry));
  }
}
