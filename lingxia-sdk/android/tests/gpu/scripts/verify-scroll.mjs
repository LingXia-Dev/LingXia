import {execFileSync} from 'node:child_process';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const [adb, serial] = process.argv.slice(2);
if (!adb || !serial) throw Error('Usage: node verify-scroll.mjs <adb> <serial>');
const run=(...args)=>execFileSync(adb,['-s',serial,...args],{timeout:30000});
const shell=(...args)=>run('shell',...args).toString();
const pkg='com.lingxia.example.gpu.dev';
const dump=()=>{shell('uiautomator','dump','/sdcard/gpu-scroll.xml');return shell('cat','/sdcard/gpu-scroll.xml');};
const node=(xml,id)=>xml.match(new RegExp(`<node[^>]*resource-id="${pkg}:id/${id}"[^>]*>`))?.[0];
const bounds=n=>n.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
const delay=ms=>new Promise(r=>setTimeout(r,ms));
async function until(check){for(let i=0;i<20;i++){const xml=dump();if(check(xml))return xml;await delay(300);}throw Error('Scroll state timed out');}
async function tap(id){const xml=await until(x=>node(x,id));const b=bounds(node(xml,id));shell('input','tap',String(Math.round((b[0]+b[2])/2)),String(Math.round((b[1]+b[3])/2)));}
shell('am','force-stop',pkg); shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);
await tap('tab-explore');
let xml=await until(x=>node(x,'moment-1'));
assert.ok(!node(xml,'moment-20'));
const viewport=bounds(node(xml,'explore-scroll'));
assert.ok(viewport[3]>viewport[1]+100 && viewport[3]<2028);
const x=Math.round((viewport[0]+viewport[2])/2), height=viewport[3]-viewport[1];
shell('input','swipe',String(x),String(Math.round(viewport[3]-height*.15)),String(x),String(Math.round(viewport[1]+height*.15)),'250');
xml=await until(x=>!node(x,'moment-1') && /resource-id="com.lingxia.example.gpu.dev:id\/moment-\d+"/.test(x));
for (let i=0;i<15 && !node(xml,'moment-20');i++) {
 shell('input','swipe',String(x),String(Math.round(viewport[3]-height*.15)),String(x),String(Math.round(viewport[1]+height*.15)),'250');
 await delay(1200); xml=dump();
}
assert.ok(node(xml,'moment-20'),'Last card is unreachable');
shell('input','keyevent','KEYCODE_HOME'); await delay(500); shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);
xml=await until(x=>node(x,'moment-20'));
const buttons=[...xml.matchAll(/<node[^>]*resource-id="com.lingxia.example.gpu.dev:id\/moment-(\d+)"[^>]*>/g)];
const button=buttons.find(([n])=>{const b=bounds(n);return b[3]-b[1]>70;});
assert.ok(button,'Expected a fully visible scrolled button');
await writeFile('target/showcase-gpu-evidence/scroll-position.png',run('exec-out','screencap','-p'));
await writeFile('target/showcase-gpu-evidence/scroll-position.xml',xml);
await tap(`moment-${button[1]}`); await tap('tab-hello');
await until(x=>node(x,'hello-count')?.includes('Hello received 1 time.'));
shell('input','keyevent','KEYCODE_HOME'); await delay(500); shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);
await until(x=>node(x,'hello-count')?.includes('Hello received 1 time.'));
const pid=shell('pidof',pkg).trim();
const logs=()=>run('logcat','-d',`--pid=${pid}`).toString();
let log=logs(); assert.match(log,/rebuild=false/); assert.match(log,/scroll fling velocity=/);
assert.doesNotMatch(log,/Render failed|FATAL EXCEPTION|UnsatisfiedLinkError|UI_SCROLL:|UI_TAG/);
await delay(2000); const frames=(logs().match(/present generation=/g)||[]).length;
await delay(5000); assert.equal((logs().match(/present generation=/g)||[]).length,frames,'Idle page kept rendering');
await writeFile('target/showcase-gpu-evidence/scroll-host.log',logs());
console.log(`PASS: clipped scroll viewport, fling, visible card ${button[1]} click, Logic count, Home/resume, no JS on scroll, idle stops`);
