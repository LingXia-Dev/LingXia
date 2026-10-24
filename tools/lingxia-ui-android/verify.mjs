import { execFileSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';

const [adb, serial] = process.argv.slice(2);
if (!adb || !serial) throw Error('Usage: node verify.mjs <adb path> <device serial>');
const run = (...args) => execFileSync(adb, ['-s', serial, ...args], { timeout: 30000, stdio: ['ignore','pipe','pipe'] });
const shell = (...args) => run('shell', ...args).toString('utf8');
const snapshot = () => JSON.parse(shell('run-as', 'com.lingxia.ui.smoke', 'cat', 'files/ui-snapshot.json'));
const countText = frame => frame.nodes.find(node => node.test_id === 'hello-count').text;
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(check) {
  const deadline = Date.now() + 30000;
  do { if (check()) return; await delay(100); } while (Date.now() < deadline);
  throw Error('Timed out waiting for device state');
}

const evidence = resolve('target/hello-ui-smoke-evidence');
await mkdir(evidence, { recursive: true });
shell('am', 'force-stop', 'com.lingxia.ui.smoke');
shell('run-as', 'com.lingxia.ui.smoke', 'rm', '-f', 'files/ui-snapshot.json');
shell('am', 'start', '-n', 'com.lingxia.ui.smoke/com.lingxia.ui.SmokeActivity');
await until(() => { try { return snapshot().backend === 'vello-vulkan' && countText(snapshot()) === 'Your first hello starts here.'; } catch { return false; } });
await delay(500);
shell('uiautomator', 'dump', '/sdcard/hello-ui-verify.xml');
const hierarchy = shell('cat', '/sdcard/hello-ui-verify.xml');
const button = hierarchy.match(/<node[^>]*text="Say hello"[^>]*>/)?.[0];
assert.ok(button?.includes('class="android.widget.Button"'), 'button must be accessible');
const bounds = button.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
const [x1,y1,x2,y2] = bounds;
shell('input', 'tap', String(Math.round((x1+x2)/2)), String(Math.round((y1+y2)/2)));
await until(() => countText(snapshot()) === 'Hello received 1 time.');
shell('input', 'tap', String(Math.round((x1+x2)/2)), String(Math.round((y1+y2)/2)));
await until(() => countText(snapshot()) === 'Hello received 2 times.');
const beforeResume = snapshot().generation;
shell('input', 'keyevent', 'KEYCODE_HOME');
await delay(400);
shell('am', 'start', '-n', 'com.lingxia.ui.smoke/com.lingxia.ui.SmokeActivity');
await until(() => snapshot().generation > beforeResume);
assert.equal(countText(snapshot()), 'Hello received 2 times.');
const before = snapshot().generation;
await delay(15000);
assert.equal(snapshot().generation, before, 'idle page must not produce frames');
await writeFile(resolve(evidence,'android-gpu.png'), run('exec-out','screencap','-p'));
await writeFile(resolve(evidence,'android-gpu-snapshot.json'), JSON.stringify(snapshot(),null,2));
await writeFile(resolve(evidence,'android-gpu-accessibility.xml'), hierarchy);
shell('input','keyevent','KEYCODE_TAB');
shell('input','keyevent','KEYCODE_ENTER');
await until(() => countText(snapshot()) === 'Hello received 3 times.');
const autoRotate = shell('settings','get','system','accelerometer_rotation').trim();
const rotation = shell('settings','get','system','user_rotation').trim();
try {
  shell('settings','put','system','accelerometer_rotation','0');
  shell('settings','put','system','user_rotation','1');
  await until(() => { const b=snapshot().nodes[0].bounds; return b[2]>b[3]; });
  const frame = snapshot();
  assert.equal(countText(frame),'Hello received 3 times.','rotation must retain the page session');
  const [,,width,height] = frame.nodes[0].bounds;
  for (const node of frame.nodes) {
    const [x,y,w,h] = node.bounds;
    assert.ok(x>=0 && y>=0 && x+w<=width+1 && y+h<=height+1, `clipped node: ${node.test_id || node.text}`);
  }
  await delay(500);
  await writeFile(resolve(evidence,'android-gpu-landscape.png'),run('exec-out','screencap','-p'));
} finally {
  shell('settings','put','system','user_rotation',rotation);
  shell('settings','put','system','accelerometer_rotation',autoRotate);
}
console.log('PASS: Vulkan, accessibility, touch and keyboard, Home/resume, idle, rotation state and layout, screenshots');
