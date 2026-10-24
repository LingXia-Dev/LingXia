// Android SDK tool resolution and checked process execution shared by the device drivers.
import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const repoRoot = resolve(fileURLToPath(new URL('../../../../../', import.meta.url)));
const windows = process.platform === 'win32';

/** `--name value` and `--flag` arguments. */
export function options(argv = process.argv.slice(2)) {
  const result = {};
  for (let i = 0; i < argv.length; i++) {
    if (!argv[i].startsWith('--')) throw Error(`Unexpected argument: ${argv[i]}`);
    const name = argv[i].slice(2), next = argv[i + 1];
    if (next === undefined || next.startsWith('--')) result[name] = true; else { result[name] = next; i++; }
  }
  return result;
}

export function androidSdk(explicit) {
  const sdk = explicit || process.env.ANDROID_SDK_ROOT || process.env.ANDROID_HOME;
  if (!sdk) throw Error('Set ANDROID_HOME/ANDROID_SDK_ROOT or pass --sdk.');
  return sdk;
}

/** A tool from the SDK with this platform's executable suffix (`.exe` binaries, `.bat` scripts on Windows). */
export function tool(sdk, relative, script = false) {
  return join(sdk, relative + (windows ? (script ? '.bat' : '.exe') : ''));
}

export const buildTools = sdk => join(sdk, 'build-tools/35.0.0');
export const androidJar = sdk => join(sdk, 'platforms/android-35/android.jar');

export function latestNdk(sdk) {
  const root = join(sdk, 'ndk');
  const versions = existsSync(root) ? readdirSync(root).sort().reverse() : [];
  if (!versions.length) throw Error('Install an Android NDK in the SDK.');
  return join(root, versions[0]);
}

/** Runs a command, inheriting output, and fails with its name on a nonzero exit. */
export function run(command, args, settings = {}) {
  const result = spawnSync(command, args, { stdio: 'inherit', shell: windows && /\.(bat|cmd)$/i.test(command), ...settings });
  if (result.error) throw result.error;
  if (result.status !== 0) throw Error(`${command} ${args[0] ?? ''} failed (${result.status})`);
  return result;
}

export function output(command, args) {
  return execFileSync(command, args, { encoding: 'utf8', shell: windows && /\.(bat|cmd)$/i.test(command) });
}
