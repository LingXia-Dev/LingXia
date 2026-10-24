import { execFileSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const [adb, serial] = process.argv.slice(2);
if (!adb || !serial) throw Error('Usage: node verify.mjs <adb> <serial>');
const run = (...args) => execFileSync(adb, ['-s', serial, ...args], { timeout: 30000 });
const shell = (...args) => run('shell', ...args).toString();
const frame = () => JSON.parse(shell('run-as', 'com.lingxia.ui.smoke', 'cat', 'files/ui-snapshot.json'));
const node = id => frame().nodes.find(n => n.test_id === id);
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(check) {
  for (let i = 0; i < 150; i++) { try { if (check()) return; } catch {} await delay(200); }
  throw Error('Device state timed out');
}
async function tap(id) {
  shell('uiautomator', 'dump', '/sdcard/showcase-gpu.xml');
  const xml = shell('cat', '/sdcard/showcase-gpu.xml');
  const element = xml.match(new RegExp(`<node[^>]*resource-id="com.lingxia.ui.smoke:id/${id}"[^>]*>`))?.[0];
  assert.ok(element, `Accessible node ${id}`);
  assert.ok(element.includes('clickable="true"'), `${id} is clickable`);
  const [x1,y1,x2,y2] = element.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
  shell('input','tap',String(Math.round((x1+x2)/2)),String(Math.round((y1+y2)/2)));
}
const evidence = 'target/showcase-gpu-evidence';
await mkdir(evidence, { recursive: true });
const capture = name => writeFile(`${evidence}/${name}.png`, run('exec-out','screencap','-p'));
shell('am','force-stop','com.lingxia.ui.smoke');
shell('run-as','com.lingxia.ui.smoke','rm','-f','files/ui-snapshot.json');
shell('am','start','-n','com.lingxia.ui.smoke/com.lingxia.ui.SmokeActivity');
await until(() => frame().backend === 'vello-vulkan' && node('showcase-gpu'));
await tap('say-hello');
await until(() => node('hello-count').text === 'Hello received 1 time.');
await capture('portrait');
await tap('theme');
await until(() => node('theme').text === 'Light');
await capture('dark');
await tap('tab-explore');
await until(() => node('explore-count').text === '1 greetings shared');
await tap('reset-count');
await until(() => node('explore-count').text === '0 greetings shared');
assert.equal(node('reset-count').on_click, null, 'reset is disabled when empty');
await capture('explore');
await tap('tab-hello');
await until(() => node('hello-count').text === 'Your first hello starts here.');
await tap('say-hello');
await until(() => node('hello-count').text === 'Hello received 1 time.');
const before = frame().generation;
shell('input','keyevent','KEYCODE_HOME');
await delay(400);
shell('am','start','-n','com.lingxia.ui.smoke/com.lingxia.ui.SmokeActivity');
await until(() => frame().generation > before);
assert.equal(node('hello-count').text, 'Hello received 1 time.');
assert.equal(node('theme').text, 'Light');
const rotate = shell('settings','get','system','accelerometer_rotation').trim();
const rotation = shell('settings','get','system','user_rotation').trim();
try {
  shell('settings','put','system','accelerometer_rotation','0');
  shell('settings','put','system','user_rotation','1');
  await until(() => { const b=node('showcase-gpu').bounds; return b[2]>b[3]; });
  const current=frame(); const [,,width,height]=current.nodes[0].bounds;
  for (const n of current.nodes) {
    const [x,y,w,h]=n.bounds;
    assert.ok(x>=0 && y>=0 && x+w<=width+1 && y+h<=height+1, `Clipped: ${n.test_id || n.text}`);
  }
  assert.equal(node('hello-count').text,'Hello received 1 time.');
  await capture('landscape');
} finally {
  shell('settings','put','system','user_rotation',rotation);
  shell('settings','put','system','accelerometer_rotation',rotate);
}
await until(() => node('showcase-gpu').bounds[2] < node('showcase-gpu').bounds[3]);
await tap('theme');
await until(() => node('theme').text === 'Dark');
const idle=frame().generation; await delay(15000);
assert.equal(frame().generation,idle,'Idle frame count');
await writeFile(`${evidence}/snapshot.json`,JSON.stringify(frame(),null,2));
await capture('portrait');
console.log('PASS: showcase Vulkan, testId accessibility, theme, tabs, reset/disabled, resume, rotation, idle');
