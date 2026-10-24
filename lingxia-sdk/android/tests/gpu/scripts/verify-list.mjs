import {execFileSync} from 'node:child_process';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const [adb,serial]=process.argv.slice(2);
if(!adb||!serial)throw Error('Usage: node verify-list.mjs <adb> <serial>');
const pkg='com.lingxia.example.gpu.dev';
const run=(...a)=>execFileSync(adb,['-s',serial,...a],{timeout:30000});
const shell=(...a)=>run('shell',...a).toString();
const delay=ms=>new Promise(r=>setTimeout(r,ms));
const dump=()=>{shell('uiautomator','dump','/sdcard/gpu-list.xml');return shell('cat','/sdcard/gpu-list.xml');};
const node=(xml,id)=>xml.match(new RegExp(`<node[^>]*resource-id="${pkg}:id/${id}"[^>]*>`))?.[0];
const bounds=n=>n.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
async function until(check){for(let i=0;i<20;i++){const xml=dump();if(check(xml))return xml;await delay(300);}throw Error('List state timed out');}
async function tap(id){const n=node(await until(x=>node(x,id)),id),b=bounds(n);shell('input','tap',String(Math.round((b[0]+b[2])/2)),String(Math.round((b[1]+b[3])/2)));}
shell('am','force-stop',pkg);shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);
await tap('tab-discover');await tap('discovery-save-1');
await until(x=>node(x,'discovery-save-1')?.includes('Saved to your day'));
await tap('discover-last');
let xml=await until(x=>node(x,'discovery-title-1000'));
assert.ok(!node(xml,'discovery-title-1'));
await writeFile('target/showcase-gpu-evidence/list-last.png',run('exec-out','screencap','-p'));
await writeFile('target/showcase-gpu-evidence/list-last.xml',xml);
await tap('discover-first');
xml=await until(x=>node(x,'discovery-save-1')?.includes('Saved to your day'));
const b=bounds(node(xml,'discover-list')), x=Math.round((b[0]+b[2])/2),h=b[3]-b[1];
for(let i=0;i<4;i++){shell('input','swipe',String(x),String(Math.round(b[3]-h*.15)),String(x),String(Math.round(b[1]+h*.15)),'250');await delay(1300);}
xml=await until(x=>!node(x,'discovery-save-1')&&/discovery-title-\d+/.test(x));
const visible=xml.match(/resource-id="[^"]*:id\/(discovery-title-\d+)"/)[1];
shell('input','keyevent','KEYCODE_HOME');await delay(500);shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);
await until(x=>node(x,visible));
await tap('discover-first');await until(x=>node(x,'discovery-save-1')?.includes('Saved to your day'));
const rotation=shell('settings','get','system','user_rotation').trim();
const auto=shell('settings','get','system','accelerometer_rotation').trim();
try {
 shell('settings','put','system','accelerometer_rotation','0');shell('settings','put','system','user_rotation','1');
 await until(x=>node(x,'discover-list')&&node(x,'discovery-title-1'));
 await writeFile('target/showcase-gpu-evidence/list-landscape.png',run('exec-out','screencap','-p'));
} finally {
 shell('settings','put','system','user_rotation',rotation);shell('settings','put','system','accelerometer_rotation',auto);
}
await until(x=>node(x,'discovery-save-1')?.includes('Saved to your day'));
await tap('discovery-hello-1');await tap('tab-hello');
await until(x=>node(x,'hello-count')?.includes('Hello received 1 time.'));
const pid=shell('pidof',pkg).trim(), logs=run('logcat','-d',`--pid=${pid}`).toString();
await writeFile('target/showcase-gpu-evidence/list-host.log',logs);
assert.doesNotMatch(logs,/UI_LIST:|UI_TAG:|UI_PROP:|Render failed|FATAL EXCEPTION/);
const counts=[...logs.matchAll(/present generation=.* nodes=(\d+)/g)].map(m=>Number(m[1]));
assert.ok(counts.length>10,'No frame node counts');assert.ok(Math.max(...counts)<200,'Offscreen rows reached layout');
console.log(`PASS: 1000 rows, first/end anchors, row state after eviction, scroll, Home/resume, rotation, Logic; max ${Math.max(...counts)} layout nodes`);
