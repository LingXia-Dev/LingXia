import typescript from "@rollup/plugin-typescript";
import terser from "@rollup/plugin-terser";

const isES5 = process.env.TARGET === "es5";
const targetPlatform = (process.env.LX_RUNTIME_PLATFORM || "all").toLowerCase();
const validPlatforms = new Set(["all", "desktop", "mobile"]);

if (!validPlatforms.has(targetPlatform)) {
  throw new Error(
    `Invalid LX_RUNTIME_PLATFORM: ${targetPlatform}. Expected one of: all, desktop, mobile`,
  );
}

function replaceRuntimePlatform() {
  const placeholder = "__LX_RUNTIME_PLATFORM__";
  const replacement = JSON.stringify(targetPlatform);
  return {
    name: "replace-runtime-platform",
    transform(code, id) {
      if (id.includes("node_modules") || !code.includes(placeholder)) {
        return null;
      }
      return {
        code: code.split(placeholder).join(replacement),
        map: null,
      };
    },
  };
}

export default {
  input: "src/index.ts",
  output: {
    file: "dist/runtime.js",
    format: "iife",
    name: "LingXiaRuntime",
    sourcemap: false,
  },
  plugins: [
    replaceRuntimePlatform(),
    typescript({
      tsconfig: "./tsconfig.json",
      target: isES5 ? "ES5" : "ES2020",
      downlevelIteration: isES5,
      noEmitHelpers: false,
      declaration: false,
      declarationDir: undefined,
    }),
    terser({
      format: {
        comments: false,
      },
    }),
  ],
};
