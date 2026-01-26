import typescript from "@rollup/plugin-typescript";
import terser from "@rollup/plugin-terser";

const isES5 = process.env.TARGET === "es5";

export default {
  input: "src/index.ts",
  output: {
    file: "dist/runtime.js",
    format: "iife",
    name: "LingXiaRuntime",
    sourcemap: false,
  },
  plugins: [
    typescript({
      tsconfig: "./tsconfig.json",
      target: isES5 ? "ES5" : "ES2020",
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
