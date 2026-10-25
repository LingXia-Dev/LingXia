// Renderer workloads run in the smoke host built from the renderer fixture app:
//   node tools/lingxia-ui-android/run.mjs --project lingxia-sdk/android/tests/gpu/fixtures/renderer --page lab
export const pkg = 'com.lingxia.ui.smoke';
export const launcher = `${pkg}/com.lingxia.ui.SmokeActivity`;
