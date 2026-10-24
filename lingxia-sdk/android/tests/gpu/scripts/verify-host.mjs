import { execFileSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
import { openPlayground } from './open-playground.mjs';
const [adb, serial] = process.argv.slice(2);
if (!adb || !serial) throw Error('Usage: node verify-host.mjs <adb> <serial>');
const pkg = 'com.lingxia.example.gpu.dev';
const launcher = `${pkg}/com.lingxia.example.gpu.MainActivity`;
const run = (...args) => execFileSync(adb, ['-s', serial, ...args], { timeout: 30000, stdio: ['ignore', 'pipe', 'pipe'] });
const shell = (...args) => run('shell', ...args).toString();
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
function hierarchy() {
  shell('uiautomator', 'dump', '/sdcard/showcase-gpu-host.xml');
  return shell('cat', '/sdcard/showcase-gpu-host.xml');
}
function node(xml, id) { return xml.match(new RegExp(`<node[^>]*resource-id="${pkg}:id/${id}"[^>]*>`))?.[0]; }
async function until(check) {
  for (let n = 0; n < 30; n++) { const xml = hierarchy(); if (check(xml)) return xml; await delay(300); }
  throw Error('Formal host state timed out');
}
async function tap(id) {
  const xml = await until(xml => node(xml, id));
  const button = node(xml, id);
  assert.ok(button.includes('clickable="true"'));
  const [x1,y1,x2,y2] = button.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
  shell('input', 'tap', String(Math.round((x1+x2)/2)), String(Math.round((y1+y2)/2)));
}
shell('am', 'force-stop', pkg);
shell('am', 'start', '-n', launcher);
await delay(2000);
await openPlayground(shell, hierarchy);
await until(xml => node(xml, 'say-hello'));
assert.match(shell('dumpsys', 'activity', 'top'), /com\.lingxia\.lxapp\.LxAppActivity/);
await tap('say-hello');
await until(xml => node(xml, 'hello-count')?.includes('Hello received 1 time.'));
const initialTheme = node(hierarchy(), 'theme').includes('text="Light"');
await tap('theme');
await until(xml => node(xml, 'theme')?.includes(initialTheme ? 'text="Dark"' : 'text="Light"'));
await tap('tab-explore');
await until(xml => node(xml, 'explore-count')?.includes('1 greetings shared'));
await tap('reset-count');
await until(xml => node(xml, 'reset-count')?.includes('enabled="false"'));
await tap('tab-hello');
await tap('say-hello');
shell('input', 'keyevent', 'KEYCODE_HOME');
await delay(500);
shell('am', 'start', '-n', launcher);
await until(xml => node(xml, 'hello-count')?.includes('Hello received 1 time.'));
const pid = shell('pidof', pkg).trim();
const logs = run('logcat', '-d', `--pid=${pid}`).toString();
assert.match(logs, /backend=vello-vulkan/);
assert.doesNotMatch(logs, /FATAL EXCEPTION|UnsatisfiedLinkError|UI_MODULE:|Render failed/);
const output = 'target/showcase-gpu-evidence';
await mkdir(output, { recursive: true });
await writeFile(`${output}/formal-host.png`, run('exec-out', 'screencap', '-p'));
await writeFile(`${output}/formal-host.xml`, hierarchy());
await writeFile(`${output}/formal-host.log`, logs);
console.log('PASS: SDK Activity, Vulkan, accessible controls, theme, tabs, disabled reset, Home/resume');
