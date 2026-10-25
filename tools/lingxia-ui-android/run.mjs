// Builds a Logic-free GPU lxapp, the core for arm64 Android and the smoke host, then launches it:
//   node run.mjs [--project <lxapp dir>] [--page <name>] [--device <serial>] [--sdk <dir>]
import { copyFileSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { androidSdk, latestNdk, options, output, repoRoot, run, tool } from '../../lingxia-sdk/android/tests/gpu/scripts/android-sdk.mjs';

const args = options();
const here = fileURLToPath(new URL('.', import.meta.url));
const project = resolve(args.project ?? join(repoRoot, 'examples/hello-ui'));
const sdk = androidSdk(args.sdk), ndk = latestNdk(sdk), windows = process.platform === 'win32';
const host = windows ? 'windows-x86_64' : process.platform === 'darwin' ? 'darwin-x86_64' : 'linux-x86_64';
const llvm = join(ndk, 'toolchains/llvm/prebuilt', host, 'bin');
const clang = join(llvm, windows ? 'aarch64-linux-android23-clang.cmd' : 'aarch64-linux-android23-clang');
const resourceDir = output(join(llvm, windows ? 'clang.exe' : 'clang'), ['-print-resource-dir']).trim().replace(/\\/g, '/');
const sysroot = join(ndk, 'toolchains/llvm/prebuilt', host, 'sysroot/usr/include').replace(/\\/g, '/');
const env = {
  ...process.env, ANDROID_HOME: sdk, ANDROID_NDK_HOME: ndk, ANDROID_NDK_ROOT: ndk, ANDROID_API_LEVEL: '23',
  CC_aarch64_linux_android: clang, CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER: clang,
  AR_aarch64_linux_android: join(llvm, windows ? 'llvm-ar.exe' : 'llvm-ar'),
  // Rong sets generic bindgen args; the target-qualified ones keep the NDK builtins.
  BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android: `-I"${resourceDir}/include" -I"${sysroot}" -I"${sysroot}/aarch64-linux-android"`,
};

run('node', [join(repoRoot, 'packages/lingxia-ui/build.mjs'), project], { cwd: repoRoot });
run('cargo', ['build', '-p', 'lingxia-ui-android', '--features', 'gpu', '--target', 'aarch64-linux-android', '--release'], { cwd: repoRoot, env });
const jniLibs = join(here, 'src/main/jniLibs/arm64-v8a');
mkdirSync(jniLibs, { recursive: true });
copyFileSync(join(repoRoot, 'target/aarch64-linux-android/release/liblingxia_ui_android.so'), join(jniLibs, 'liblingxia_ui_android.so'));
run(join(repoRoot, 'lingxia-sdk/android', windows ? 'gradlew.bat' : 'gradlew'), ['-p', here, `-PuiAssets=${project}/dist`, 'assembleDebug', '--console=plain'], { env });
const adb = tool(sdk, 'platform-tools/adb'), selector = args.device ? ['-s', args.device] : [];
run(adb, [...selector, 'install', '-r', join(here, 'build/outputs/apk/debug/LingXiaUiSmoke-debug.apk')]);
run(adb, [...selector, 'shell', 'am', 'start', '-n', 'com.lingxia.ui.smoke/com.lingxia.ui.SmokeActivity', ...(args.page ? ['--es', 'page', args.page] : [])]);
