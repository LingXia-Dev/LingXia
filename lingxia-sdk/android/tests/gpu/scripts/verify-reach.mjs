import {execFileSync} from 'node:child_process';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const [adb,serial]=process.argv.slice(2),pkg='com.lingxia.example.gpu.dev';
if(!adb||!serial)throw Error('Usage: node verify-reach.mjs <adb> <serial>');
const run=(...a)=>execFileSync(adb,['-s',serial,...a],{timeout:30000});
const shell=(...a)=>run('shell',...a).toString();
const delay=ms=>new Promise(r=>setTimeout(r,ms));
const dump=()=>{shell('uiautomator','dump','/sdcard/gpu-reach.xml');return shell('cat','/sdcard/gpu-reach.xml');};
const node=(xml,id)=>xml.match(new RegExp(`<node[^>]*resource-id="${pkg}:id/${id}"[^>]*>`))?.[0];
async function until(check){for(let i=0;i<20;i++){const xml=dump();if(check(xml))return xml;await delay(300);}throw Error('Endpoint state timed out');}
async function tap(id){const b=node(await until(x=>node(x,id)),id).match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);shell('input','tap',String(Math.round((b[0]+b[2])/2)),String(Math.round((b[1]+b[3])/2)));}
shell('am','force-stop',pkg);shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);
await tap('tab-discover');
const pid=shell('pidof',pkg).trim(),log=()=>run('logcat','-d',`--pid=${pid}`).toString();
async function idle(){await delay(2000);const before=(log().match(/present generation=/g)||[]).length;await delay(3000);assert.equal((log().match(/present generation=/g)||[]).length,before,'Edge callback caused repeated frames');}
await until(x=>node(x,'discover-status')?.includes('More small moments below.'));await idle();
for(let i=0;i<2;i++){
 await tap('discover-last');await until(x=>node(x,'discover-status')?.includes('You are up to date.')&&node(x,'discovery-title-1000'));await idle();
 await tap('discover-first');await until(x=>node(x,'discover-status')?.includes('More small moments below.')&&node(x,'discovery-title-1'));await idle();
}
await writeFile('target/showcase-gpu-evidence/reach-host.log',log());
await writeFile('target/showcase-gpu-evidence/reach-start.png',run('exec-out','screencap','-p'));
assert.doesNotMatch(log(),/UI_LIST:|UI_SCROLL:|UI_PROP:|Render failed|FATAL EXCEPTION/);
console.log('PASS: start/end callbacks update local state only on edge entry; repeated jumps work; idle stops');
