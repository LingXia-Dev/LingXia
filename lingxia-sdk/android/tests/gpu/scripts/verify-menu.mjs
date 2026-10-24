// Pixel 3 / Android 12: operate the real floating toolbar through UIAutomator bounds.
import {execFileSync} from 'node:child_process';
import {mkdirSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
const [adb,serial]=process.argv.slice(2),pkg='com.lingxia.example.gpu.dev';
const run=(...args)=>execFileSync(adb,['-s',serial,...args],{timeout:30000});
const shell=(...args)=>run('shell',...args).toString();
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const xml=()=>{shell('uiautomator','dump','/sdcard/gpu-menu.xml');return shell('cat','/sdcard/gpu-menu.xml');};
const node=(tree,id)=>tree.match(new RegExp(`<node[^>]*resource-id="${pkg}:id/${id}"[^>]*>`))?.[0];
const windows=()=>JSON.parse(shell('CLASSPATH=/data/local/tmp/gpu-windows-probe.jar','app_process','/system/bin','GpuWindowsProbe'));
const menu=text=>windows().find(node=>node.text===text);
const bounds=target=>target.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
async function until(check){for(let i=0;i<10;i++){const tree=xml();if(check(tree))return tree;await delay(200);}throw Error('Menu state timed out');}
async function waitMenu(text){for(let i=0;i<5;i++){const found=menu(text);if(found)return found;await delay(200);}throw Error(`Missing menu: ${text}`);}
async function tapLabel(text){const [x,y,r,b]=(await waitMenu(text)).bounds;shell('input','tap',String(Math.round((x+r)/2)),String(Math.round((y+b)/2)));await delay(500);}
async function inputAt(long=false){const [x,y,r,b]=bounds(node(await until(t=>node(t,'greeting-input')),'greeting-input'));const px=x+70,py=Math.round((y+b)/2);if(long)shell('input','swipe',String(px),String(py),String(px),String(py),'850');else shell('input','tap',String(px),String(py));await delay(700);}
async function expect(value){await until(t=>node(t,'greeting-input')?.includes(`text="${value}"`));}
const out='target/gpu-menu-evidence';mkdirSync(out,{recursive:true});
function capture(name){writeFileSync(`${out}/${name}.png`,run('exec-out','screencap','-p'));writeFileSync(`${out}/${name}.xml`,xml());writeFileSync(`${out}/${name}-windows.json`,JSON.stringify(windows()));}
shell('am','force-stop',pkg);shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);
await inputAt();shell('input','text','hello%sworld');await expect('hello world');
await inputAt(true);await waitMenu('Copy');await waitMenu('Cut');await waitMenu('Paste');capture('selected-menu');
const activePid=shell('pidof',pkg).trim();
const presents=()=>run('logcat','-d',`--pid=${activePid}`).toString().split('\n').filter(line=>line.includes('present generation=')).length;
const idle=presents();await delay(1500);assert.equal(presents(),idle,'stationary selection toolbar must not trigger a frame loop');
await tapLabel('Copy');await expect('hello world');assert(!menu('Cut'));
await inputAt(true);await tapLabel('Cut');await expect(' world');
await inputAt(true);await tapLabel('Paste');await expect(' hello');
await inputAt(true);
// Select all may be in the platform overflow depending on available width.
if(!menu('Select all')){
 const overflow=windows().find(node=>node.description==='More options');assert(overflow,'missing toolbar overflow');
 const [x,y,r,b]=overflow.bounds;shell('input','tap',String(Math.round((x+r)/2)),String(Math.round((y+b)/2)));await delay(300);
}
await tapLabel('Select all');capture('all-selected');shell('input','text','done');await expect('done');assert(!menu('Cut'));
await inputAt(true);await waitMenu('Copy');shell('input','keyevent','KEYCODE_HOME');await delay(400);
shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);await until(t=>node(t,'greeting-input'));assert(!menu('Cut'));
capture('resumed');
const pid=shell('pidof',pkg).trim(),log=run('logcat','-d',`--pid=${pid}`).toString();
assert.doesNotMatch(log,/FATAL EXCEPTION|UnsatisfiedLinkError|Render failed/);writeFileSync(`${out}/device.log`,log);
console.log('PASS: native floating copy/cut/paste/select-all menu, typing dismissal and Home/resume cleanup on GPU input');
