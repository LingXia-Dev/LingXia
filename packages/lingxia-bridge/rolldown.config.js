import path from "node:path";
import { defineConfig } from "rolldown";
import esbuild from "rollup-plugin-esbuild";

const targetPlatform = (process.env.LX_RUNTIME_PLATFORM || "all").toLowerCase();
const outputFile = process.env.RUNTIME_OUTPUT || "runtime.js";
const outputDir = process.env.RUNTIME_OUTPUT_DIR || "dist";
const outputPath = path.join(outputDir, outputFile);
const validPlatforms = new Set(["all", "desktop", "mobile"]);

if (!validPlatforms.has(targetPlatform)) {
  throw new Error(
    `Invalid LX_RUNTIME_PLATFORM: ${targetPlatform}. Expected one of: all, desktop, mobile`,
  );
}

export default defineConfig({
  input: "src/index.ts",
  output: {
    file: outputPath,
    format: "iife",
    name: "LingXiaRuntime",
    sourcemap: false,
  },
  plugins: [
    esbuild({
      tsconfig: "./tsconfig.json",
      target: "es2020",
      define: {
        __LX_RUNTIME_PLATFORM__: JSON.stringify(targetPlatform),
      },
      minify: true,
      sourceMap: false,
    }),
  ],
});
