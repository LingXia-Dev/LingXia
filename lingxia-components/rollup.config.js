import terser from "@rollup/plugin-terser";

export default {
  input: "dist/index.js",
  output: [
    { file: "dist/bundle.js", format: "es", sourcemap: true }
  ],
  plugins: [terser()]
};
