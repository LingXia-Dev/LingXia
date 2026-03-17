import terser from "@rollup/plugin-terser";

export default {
  input: "dist/index.js",
  output: [
    {
      dir: "dist/bundle",
      format: "es",
      sourcemap: true,
      preserveModules: false
    }
  ],
  plugins: [terser()]
};
