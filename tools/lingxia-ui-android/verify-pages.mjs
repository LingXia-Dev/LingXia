import { execFileSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const [adb, serial] = process.argv.slice(2);
if (!adb || !serial) throw Error('Usage: node verify-pages.mjs <adb> <serial>');
const run = (...args) => execFileSync(adb, ['-s', serial, ...args], { timeout: 30000, stdio: ['ignore','pipe','pipe'] });
const shell = (...args) => run('shell', ...args).toString();
const snapshot = () => JSON.parse(shell('run-as','com.lingxia.ui.smoke','cat','files/ui-snapshot.json'));
const delay = ms => new Promise(resolve => setTimeout(resolve,ms));
async function until(check) {
  for (let n=0;n<150;n++) { try { if (check()) return; } catch {} await delay(200); }
  throw Error('Page state timed out');
}
const count = () => snapshot().nodes.find(n => n.test_id === 'hello-count').text;
async function tap(text) {
  shell('uiautomator','dump','/sdcard/ui-pages.xml');
  const xml=shell('cat','/sdcard/ui-pages.xml');
  const button=xml.match(new RegExp(`<node[^>]*text="${text}"[^>]*>`, 'i'))?.[0];
  assert.ok(button?.includes('clickable="true"'),`Clickable ${text}`);
  const [x1,y1,x2,y2]=button.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
  shell('input','tap',String(Math.round((x1+x2)/2)),String(Math.round((y1+y2)/2)));
}
shell('am','force-stop','com.lingxia.ui.smoke');
shell('run-as','com.lingxia.ui.smoke','rm','-f','files/ui-snapshot.json');
shell('am','start','-n','com.lingxia.ui.smoke/com.lingxia.ui.SmokeActivity','--ez','multiPage','true');
await until(() => snapshot().page===0 && snapshot().backend==='vello-vulkan');
await tap('Say hello'); await until(() => count()==='Hello received 1 time.');
await tap('Switch page'); await until(() => snapshot().page===1);
assert.equal(count(),'Your first hello starts here.','Second context must start independently');
assert.equal(snapshot().backend,'vello-vulkan');
await tap('Say hello'); await until(() => count()==='Hello received 1 time.');
await tap('Say hello'); await until(() => count()==='Hello received 2 times.');
await tap('Switch page'); await until(() => snapshot().page===0);
assert.equal(count(),'Hello received 1 time.','First context survives detachment');
await mkdir('target/showcase-gpu-evidence',{recursive:true});
await writeFile('target/showcase-gpu-evidence/page-0.png',run('exec-out','screencap','-p'));
await tap('Switch page'); await until(() => snapshot().page===1);
assert.equal(count(),'Hello received 2 times.','Second context survives detachment');
await writeFile('target/showcase-gpu-evidence/page-1.png',run('exec-out','screencap','-p'));
shell('am','force-stop','com.lingxia.ui.smoke');
shell('run-as','com.lingxia.ui.smoke','rm','-f','files/ui-snapshot.json');
shell('am','start','-n','com.lingxia.ui.smoke/com.lingxia.ui.SmokeActivity');
await until(() => snapshot().page===0 && count()==='Your first hello starts here.');
console.log('PASS: two Vulkan pages share a View executor, retain independent state across surface detachment, and recreate cleanly');
