// Physical Android keyboard shortcuts against the installed GPU showcase.
import {execFileSync} from 'node:child_process';
import {mkdirSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
const [adb,serial]=process.argv.slice(2),pkg='com.lingxia.example.gpu.dev';
const run=(...args)=>execFileSync(adb,['-s',serial,...args],{timeout:30000});
const shell=(...args)=>run('shell',...args).toString();
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const xml=()=>{shell('uiautomator','dump','/sdcard/gpu-clipboard.xml');return shell('cat','/sdcard/gpu-clipboard.xml');};
const node=(tree,id)=>tree.match(new RegExp(`<node[^>]*resource-id="${pkg}:id/${id}"[^>]*>`))?.[0];
async function until(check){for(let i=0;i<12;i++){const tree=xml();if(check(tree))return tree;await delay(200);}throw Error('Clipboard state timed out');}
// Android 12's input keycombination injects metaState=0; use explicit Ctrl KeyEvents.
const shortcut=key=>shell('CLASSPATH=/data/local/tmp/gpu-shortcut-probe.jar','app_process','/system/bin','GpuShortcutProbe',key);
const out='target/gpu-clipboard-evidence';mkdirSync(out,{recursive:true});
function capture(name){writeFileSync(`${out}/${name}.png`,run('exec-out','screencap','-p'));writeFileSync(`${out}/${name}.xml`,xml());}
shell('am','force-stop',pkg);shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);
const tree=await until(t=>node(t,'greeting-input'));
const [x,y,r,b]=node(tree,'greeting-input').match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
shell('input','tap',String(Math.round((x+r)/2)),String(Math.round((y+b)/2)));await delay(800);
shell('input','text','Clipboard42');
await until(t=>node(t,'greeting-input')?.includes('text="Clipboard42"'));
shortcut('A');await delay(400);capture('selected');
shortcut('C');shortcut('X');
await until(t=>node(t,'greeting-input')?.includes('text="Write a little hello"') && node(t,'greeting-preview')?.includes('Make it personal'));
capture('cut');shortcut('V');
await until(t=>node(t,'greeting-input')?.includes('text="Clipboard42"'));
shortcut('V');
await until(t=>node(t,'greeting-input')?.includes('text="Clipboard42Clipboard42"'));
shortcut('A');shell('input','text','replacement');
await until(t=>node(t,'greeting-input')?.includes('text="replacement"'));
shortcut('A');shortcut('V');
await until(t=>node(t,'greeting-input')?.includes('text="Clipboard42"'));
capture('pasted');shell('input','keyevent','KEYCODE_ENTER');
await until(t=>node(t,'greeting-sent')?.includes('Saved: Clipboard42'));
await delay(500);assert.match(shell('dumpsys','input_method'),/mInputShown=false/);
capture('submitted');
const pid=shell('pidof',pkg).trim(),log=run('logcat','-d',`--pid=${pid}`).toString();
assert.doesNotMatch(log,/FATAL EXCEPTION|UnsatisfiedLinkError|Render failed/);
writeFileSync(`${out}/device.log`,log);
console.log('PASS: GPU select-all/copy/cut/paste shortcuts, repeated insertion, replacement and submit on physical Android');
