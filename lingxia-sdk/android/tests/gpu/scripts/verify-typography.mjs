import {execFileSync} from 'node:child_process';
import {mkdirSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
const [adb,serial]=process.argv.slice(2),pkg='com.lingxia.example.gpu.dev';
const run=(...args)=>execFileSync(adb,['-s',serial,...args],{timeout:30000});
const shell=(...args)=>run('shell',...args).toString();
const delay=ms=>new Promise(r=>setTimeout(r,ms));
const xml=()=>{shell('uiautomator','dump','/sdcard/gpu-typography.xml');return shell('cat','/sdcard/gpu-typography.xml');};
const node=(tree,id)=>tree.match(new RegExp(`<node[^>]*resource-id="${pkg}:id/${id}"[^>]*>`))?.[0];
async function tap(id){
 const n=node(xml(),id);assert.ok(n,id);
 const [x,y,r,b]=n.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
 shell('input','tap',String((x+r)/2),String((y+b)/2));await delay(500);
}
shell('am','force-stop',pkg);shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);await delay(1500);
if(node(xml(),'theme')?.includes('text="Light"'))await tap('theme');
await tap('note-mode');await tap('greeting-input');
shell('input','text','iiii');shell('input','keyevent','KEYCODE_ENTER');shell('input','text','WWWW');
shell('input','keyevent','KEYCODE_BACK');await delay(700);
const tree=xml();assert.ok(node(tree,'greeting-input')?.includes('iiii&#10;WWWW'));
const out='target/gpu-typography-evidence';mkdirSync(out,{recursive:true});
writeFileSync(`${out}/mono.xml`,tree);writeFileSync(`${out}/mono.png`,run('exec-out','screencap','-p'));
console.log('PASS typography sample: multiline input retained on physical device');
