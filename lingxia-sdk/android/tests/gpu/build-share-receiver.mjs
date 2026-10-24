// Builds and installs the test share receiver: node build-share-receiver.mjs --serial <device> [--sdk <dir>]
import { mkdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { androidJar, androidSdk, buildTools, options, repoRoot, run, tool } from './scripts/android-sdk.mjs';

const args = options();
if (!args.serial) throw Error('Pass --serial <device>.');
const sdk = androidSdk(args.sdk), tools = buildTools(sdk), windows = process.platform === 'win32';
const source = fileURLToPath(new URL('./receiver/', import.meta.url));
const out = join(repoRoot, 'target/gpu-share-receiver');
mkdirSync(join(out, 'classes'), { recursive: true }); mkdirSync(join(out, 'dex'), { recursive: true });
run('javac', ['-classpath', androidJar(sdk), '-d', join(out, 'classes'), join(source, 'Receiver.java')]);
run(join(tools, windows ? 'd8.bat' : 'd8'), ['--lib', androidJar(sdk), '--output', join(out, 'dex'), join(out, 'classes/com/lingxia/test/receiver/Receiver.class')]);
run(join(tools, windows ? 'aapt2.exe' : 'aapt2'), ['link', '-I', androidJar(sdk), '--manifest', join(source, 'AndroidManifest.xml'), '-o', join(out, 'unsigned.apk')]);
run('jar', ['uf', join(out, 'unsigned.apk'), '-C', join(out, 'dex'), 'classes.dex']);
run(join(tools, windows ? 'zipalign.exe' : 'zipalign'), ['-f', '4', join(out, 'unsigned.apk'), join(out, 'aligned.apk')]);
run(join(tools, windows ? 'apksigner.bat' : 'apksigner'), ['sign', '--ks', join(homedir(), '.android/debug.keystore'), '--ks-pass', 'pass:android',
  '--key-pass', 'pass:android', '--out', join(out, 'receiver.apk'), join(out, 'aligned.apk')]);
run(tool(sdk, 'platform-tools/adb'), ['-s', args.serial, 'install', '-r', join(out, 'receiver.apk')]);
