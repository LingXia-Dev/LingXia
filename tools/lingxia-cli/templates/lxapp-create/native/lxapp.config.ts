const nativeOut =
  "{{FRAMEWORK}}" === "html"
    ? "__lingxia/native.js"
    : "src/generated/native.ts";

export default {
  native: {
    rustDir: "{{NATIVE_RUST_DIR}}",
    out: nativeOut,
  },
};
