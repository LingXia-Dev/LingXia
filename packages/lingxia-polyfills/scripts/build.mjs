// Build dist/polyfills.es5.js from src/es5.js.
//
// Whitespace-only minify (terser `compress: false`) — see the warning in
// src/es5.js for why we cannot run compress over polyfill feature detects.
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { minify } from "terser";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const packageDir = path.resolve(__dirname, "..");
const srcFile = path.join(packageDir, "src", "es5.js");
const distDir = path.join(packageDir, "dist");
const outputFile = path.join(distDir, "polyfills.es5.js");

await fs.mkdir(distDir, { recursive: true });

const source = await fs.readFile(srcFile, "utf8");
const result = await minify(source, {
  compress: false,
  mangle: false,
  format: { comments: false },
});
if (!result.code) {
  throw new Error("terser produced no output");
}
await fs.writeFile(outputFile, result.code);
console.log(`[polyfills] wrote ${outputFile} (${result.code.length} bytes)`);
