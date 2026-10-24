// Runs core-suite.json against an installed APK and records results.json:
//   node run-core-suite.mjs --apk <path> --serial <device> [--sdk <dir>] [--out <dir>]
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync, openSync, closeSync } from 'node:fs';
import { spawn, execFileSync } from 'node:child_process';
import { join, resolve } from 'node:path';
import { androidSdk, options, output, repoRoot, run, tool } from './scripts/android-sdk.mjs';
import { compileProbes } from './run-probe.mjs';

const args = options();
if (!args.apk || !/^[A-Za-z0-9._:-]+$/.test(args.serial ?? '')) throw Error('Pass --apk <path> and --serial <device>.');
const sdk = androidSdk(args.sdk), serial = args.serial;
const adb = tool(sdk, 'platform-tools/adb');
const shell = (...command) => output(adb, ['-s', serial, 'shell', ...command]).trim();
const manifest = JSON.parse(readFileSync(new URL('./core-suite.json', import.meta.url), 'utf8'));
const stamp = new Date().toISOString().replace(/[-:]/g, '').replace('T', '-').slice(0, 15);
const out = resolve(args.out ?? join(repoRoot, `target/gpu-core-${stamp}`));
mkdirSync(out, { recursive: true });

const expected = createHash('sha256').update(readFileSync(args.apk)).digest('hex');
const remote = shell('pm', 'path', manifest.package).split('\n')[0].trim().slice('package:'.length);
if (shell('sha256sum', remote).split(/\s+/)[0] !== expected) throw Error('Installed APK differs from the supplied artifact; install that APK before testing.');
const { jar } = compileProbes(sdk);
run(adb, ['-s', serial, 'push', jar, '/data/local/tmp/gpu-independent-probe.jar']);

const evidence = {
  startedAt: new Date().toISOString(), apkSha256: expected, serial,
  model: shell('getprop', 'ro.product.model'), os: shell('getprop', 'ro.build.version.release'),
  viewport: shell('wm', 'size'), fontScale: shell('settings', 'get', 'system', 'font_scale'),
  package: shell('dumpsys', 'package', manifest.package).split('\n').filter(line => /versionCode=|versionName=/.test(line)).map(line => line.trim()),
  sourceHead: execFileSync('git', ['-C', repoRoot, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
  dirtyPaths: execFileSync('git', ['-C', repoRoot, 'status', '--porcelain'], { encoding: 'utf8' }).split('\n').filter(Boolean),
  scenarios: [], status: 'running',
};
const save = () => writeFileSync(join(out, 'results.json'), JSON.stringify(evidence, null, 2));

function scenario({ probe, timeoutSeconds }) {
  if (!/^Gpu[A-Za-z]+Probe$/.test(probe)) throw Error('Invalid probe name');
  shell('am', 'force-stop', manifest.package);
  shell('monkey', '-p', manifest.package, '1');
  const log = join(out, `${probe}.log`), stdout = openSync(log, 'w'), stderr = openSync(join(out, `${probe}.stderr.log`), 'w');
  const child = spawn(adb, ['-s', serial, 'shell', 'CLASSPATH=/data/local/tmp/gpu-independent-probe.jar', 'app_process', '/system/bin', probe],
    { stdio: ['ignore', stdout, stderr] });
  return new Promise(done => {
    const timer = setTimeout(() => child.kill(), timeoutSeconds * 1000);
    child.on('exit', code => {
      clearTimeout(timer); closeSync(stdout); closeSync(stderr);
      const passed = code === 0 && /^PASS /m.test(readFileSync(log, 'utf8'));
      done({ probe, status: passed ? 'pass' : 'fail', exitCode: code, log });
    });
  });
}

try {
  for (const entry of manifest.scenarios) {
    const result = await scenario(entry);
    evidence.scenarios.push(result); save();
    if (result.status !== 'pass') throw Error(`Probe failed or exceeded deadline: ${entry.probe}`);
    console.log(`PASS ${entry.probe}`);
  }
  evidence.status = 'pass';
} catch (error) {
  evidence.status = 'fail'; evidence.error = error.message; process.exitCode = 1; console.error(error.message);
} finally {
  evidence.finishedAt = new Date().toISOString(); save();
}
