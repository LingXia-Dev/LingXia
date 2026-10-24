import {execFileSync} from 'node:child_process';
import {mkdir, writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';

const [adb, serial, pkg, id] = process.argv.slice(2);
if (!id) throw Error('Usage: node verify-feedback.mjs <adb> <serial> <package> <visible button testId> (requires Python + Pillow)');
const run = (...args) => execFileSync(adb, ['-s', serial, ...args], {timeout: 30000});
const shell = (...args) => run('shell', ...args).toString();
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const evidence = 'target/gpu-feedback-evidence';
await mkdir(evidence, {recursive: true});
shell('uiautomator', 'dump', '/sdcard/gpu-feedback.xml');
const xml = shell('cat', '/sdcard/gpu-feedback.xml');
const button = xml.match(/<node\b[^>]*>/g)?.find(n => n.includes(`resource-id="${pkg}:id/${id}"`));
assert.ok(button?.includes('clickable="true"'), 'Expected a visible enabled button');
const [left, top, right, bottom] = button.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
const x = Math.round(left + (right-left)*0.2), y = Math.round((top+bottom)/2);
const capture = async name => {
  const path = `${evidence}/${name}.png`;
  await writeFile(path, run('exec-out', 'screencap', '-p'));
  return JSON.parse(execFileSync('python', ['-c', 'from PIL import Image; import sys,json; print(json.dumps(Image.open(sys.argv[1]).convert("RGB").getpixel((int(sys.argv[2]),int(sys.argv[3])))))', path, String(x), String(y)]).toString());
};
const before = await capture('before');
try {
  shell('input', 'motionevent', 'DOWN', String(x), String(y));
  await delay(300);
  const pressed = await capture('pressed');
  assert.ok(pressed.every((v,i) => v <= before[i]) && pressed.some((v,i) => before[i]-v >= 8), 'Pressed feedback must visibly darken the button');
} finally {
  shell('input', 'motionevent', 'UP', String(x), String(y));
}
await delay(500);
assert.deepEqual(await capture('released'), before, 'Feedback must disappear after release');
console.log('PASS: physical pressed feedback and release');
