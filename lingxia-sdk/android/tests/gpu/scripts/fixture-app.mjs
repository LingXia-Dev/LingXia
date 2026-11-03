// Renderer workloads run in the smoke host built from the renderer fixture app:
//   node tools/lingxia-ui-android/run.mjs --project lingxia-sdk/android/tests/gpu/fixtures/renderer --page lab
export const pkg = 'com.lingxia.view.smoke';
export const launcher = `${pkg}/com.lingxia.view.SmokeActivity`;
