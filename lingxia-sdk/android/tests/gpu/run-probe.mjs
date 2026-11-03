// Compiles the UiAutomation probes to one dex jar and runs one on a device:
//   node run-probe.mjs --probe GpuStreamProbe --serial <device> [--sdk <dir>] [--compile-only]
// probe-targets.json names each probe's app; smoke-host apps are launched on their page first.
import { mkdirSync, readdirSync, readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { androidJar, androidSdk, buildTools, options, repoRoot, run, tool } from './scripts/android-sdk.mjs';

export function compileProbes(sdk) {
  const sources = fileURLToPath(new URL('./probes/', import.meta.url));
  const classes = join(repoRoot, 'target/gpu-independent-probe-classes');
  const input = join(repoRoot, 'target/gpu-independent-probe-input.jar');
  const jar = join(repoRoot, 'target/gpu-independent-probe.jar');
  mkdirSync(classes, { recursive: true });
  const files = readdirSync(sources).filter(name => name.endsWith('.java')).map(name => join(sources, name));
  run('javac', ['-encoding', 'UTF-8', '-classpath', androidJar(sdk), '-d', classes, ...files]);
  // One archive keeps d8's command line short on Windows.
  run('jar', ['cf', input, '-C', classes, '.']);
  run(join(buildTools(sdk), process.platform === 'win32' ? 'd8.bat' : 'd8'), ['--lib', androidJar(sdk), '--output', jar, input]);
  return { jar, count: files.length };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = options();
  const probe = args.probe ?? 'GpuStreamProbe';
  if (!/^Gpu[A-Za-z]+Probe$/.test(probe) || !existsSync(new URL(`./probes/${probe}.java`, import.meta.url))) throw Error(`Unknown probe: ${probe}`);
  const sdk = androidSdk(args.sdk);
  const { jar, count } = compileProbes(sdk);
  if (args['compile-only']) { console.log(`Compiled ${count} Android test probes`); process.exit(0); }
  const serial = args.serial ?? process.env.ANDROID_SERIAL;
  if (!serial) throw Error('Pass --serial or set ANDROID_SERIAL.');
  const adb = tool(sdk, 'platform-tools/adb');
  const targets = JSON.parse(readFileSync(new URL('./probe-targets.json', import.meta.url), 'utf8'));
  const [app, page] = (targets.targets[probe] ?? targets.default).split('/');
  const { package: pkg, project } = targets.apps[app];
  run(adb, ['-s', serial, 'push', jar, '/data/local/tmp/gpu-independent-probe.jar']);
  if (pkg === 'com.lingxia.view.smoke') {
    console.log(`${probe} expects the smoke host built with --project ${project}`);
    run(adb, ['-s', serial, 'shell', 'am', 'force-stop', pkg]);
    run(adb, ['-s', serial, 'shell', 'am', 'start', '-W', '-n', `${pkg}/com.lingxia.view.SmokeActivity`, ...(page ? ['--es', 'page', page] : [])]);
  }
  run(adb, ['-s', serial, 'shell', `GPU_PACKAGE=${pkg} CLASSPATH=/data/local/tmp/gpu-independent-probe.jar app_process /system/bin ${probe}`]);
}
