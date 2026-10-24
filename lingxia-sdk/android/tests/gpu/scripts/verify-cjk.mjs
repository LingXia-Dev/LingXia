import {execFileSync} from 'node:child_process';
import {mkdirSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
// Run from the repo root with adb path and serial; enable Simplified Chinese Pinyin first.
// Pixel 3 / Android 12, Gboard 10.8.03.382062350. Restore the original layouts afterward.
const [adb,serial]=process.argv.slice(2),pkg='com.lingxia.example.gpu.dev';
assert(adb&&serial,'usage: node verify-cjk.mjs <adb> <serial>');
const run=(...args)=>execFileSync(adb,['-s',serial,...args],{timeout:30000});
const shell=(...args)=>run('shell',...args).toString();
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const xml=()=>{shell('uiautomator','dump','/sdcard/gpu-cjk.xml');return shell('cat','/sdcard/gpu-cjk.xml');};
const node=(tree,id)=>tree.match(new RegExp(`<node[^>]*resource-id="${pkg}:id/${id}"[^>]*>`))?.[0];
const keyboard=()=>JSON.parse(shell('CLASSPATH=/data/local/tmp/gpu-windows-probe.jar','app_process','/system/bin','GpuWindowsProbe','com.google.android.inputmethod.latin'));
const tap=b=>shell('input','tap',String(Math.round((b[0]+b[2])/2)),String(Math.round((b[1]+b[3])/2)));
async function until(check){for(let i=0;i<10;i++){const tree=xml();if(check(tree))return tree;await delay(200);}throw Error('CJK state timed out');}
async function focus(){const t=node(await until(t=>node(t,'greeting-input')),'greeting-input');const b=t.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);tap([b[2]-30,b[1],b[2]-15,b[3]]);await delay(900);}
async function typePinyin(value){const keys=keyboard();for(const char of value){const key=keys.find(n=>n.description===char&&n.bounds[1]>1352);assert(key,`missing key ${char}`);tap(key.bounds);await delay(100);}}
async function candidate(value){for(let i=0;i<5;i++){const match=keyboard().find(n=>n.description===value);if(match){tap(match.bounds);await delay(400);return;}await delay(200);}throw Error(`Missing candidate ${value}`);}
const out='target/gpu-cjk-evidence';mkdirSync(out,{recursive:true});
function capture(name){writeFileSync(`${out}/${name}.png`,run('exec-out','screencap','-p'));writeFileSync(`${out}/${name}.xml`,xml());writeFileSync(`${out}/${name}-keyboard.json`,JSON.stringify(keyboard()));}
shell('am','force-stop',pkg);shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);await focus();
assert(keyboard().some(n=>n.description==='\u5b8c\u6210'),'select Chinese Pinyin before running');
await typePinyin('nihao');
assert.match(node(xml(),'greeting-preview'),/Make it personal/);capture('pinyin-candidates');
await candidate('\u4f60\u597d');
await until(t=>node(t,'greeting-input')?.includes('text="\u4f60\u597d"')&&node(t,'greeting-preview')?.includes('Your note: \u4f60\u597d'));
capture('committed-chinese');
await typePinyin('shijie');
assert.match(node(xml(),'greeting-preview'),/Your note: \u4f60\u597d/);capture('second-preedit');
shell('input','keyevent','KEYCODE_HOME');await delay(400);shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);
await until(t=>node(t,'greeting-input')?.includes('text="\u4f60\u597d"'));
await focus();await typePinyin('shijie');await candidate('\u4e16\u754c');
await until(t=>node(t,'greeting-input')?.includes('text="\u4f60\u597d\u4e16\u754c"'));
capture('phrase');
// This Gboard clears suggestions on the first Delete; native Settings EditText agrees.
// Assert the intermediate state rather than retrying until text happens to match.
const deleteKey=keyboard().find(n=>n.description==='\u5220\u9664');await delay(700);tap(deleteKey.bounds);await delay(500);assert.match(node(xml(),'greeting-input'),/text=\"你好世界\"/);capture('first-delete');tap(deleteKey.bounds);await until(t=>node(t,'greeting-input')?.includes('text="\u4f60\u597d\u4e16"'));
const doneKey=keyboard().find(n=>n.description==='\u5b8c\u6210');await delay(700);tap(doneKey.bounds);
await until(t=>node(t,'greeting-sent')?.includes('Saved: \u4f60\u597d\u4e16'));capture('submitted');
const pid=shell('pidof',pkg).trim(),log=run('logcat','-d',`--pid=${pid}`).toString();
assert.doesNotMatch(log,/FATAL EXCEPTION|UnsatisfiedLinkError|Render failed/);writeFileSync(`${out}/device.log`,log);
console.log('PASS: physical Gboard Pinyin candidates, private preedit, Chinese commit, Home cancellation, deletion and submit');
