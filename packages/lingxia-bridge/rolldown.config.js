import path from "node:path";
import { defineConfig } from "rolldown";

const outputFile = process.env.RUNTIME_OUTPUT || "bridge-runtime.js";
const outputDir = process.env.RUNTIME_OUTPUT_DIR || "dist";
const outputPath = path.join(outputDir, outputFile);

export default defineConfig({
  input: "src/index.ts",
  output: {
    file: outputPath,
    format: "iife",
    name: "__LingXiaBridgeBundle",
    sourcemap: false,
  },
});
