// Physical Pixel 3: positions use the input's measured bounds and 440 dpi padding.
import {execFileSync} from 'node:child_process';
import {mkdirSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
const [adb,serial]=process.argv.slice(2),pkg='com.lingxia.example.gpu.dev';
const run=(...args)=>execFileSync(adb,['-s',serial,...args],{timeout:30000});
const shell=(...args)=>run('shell',...args).toString();
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const xml=()=>{shell('uiautomator','dump','/sdcard/gpu-touch.xml');return shell('cat','/sdcard/gpu-touch.xml');};
const node=(tree,id='greeting-input')=>tree.match(new RegExp(`<node[^>]*resource-id="${pkg}:id/${id}"[^>]*>`))?.[0];
async function until(check){for(let i=0;i<10;i++){const tree=xml();if(check(tree))return tree;await delay(200);}throw Error('Touch state timed out');}
const bounds=target=>target.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
const shortcut=key=>shell('CLASSPATH=/data/local/tmp/gpu-shortcut-probe.jar','app_process','/system/bin','GpuShortcutProbe',key);
const out='target/gpu-touch-evidence';mkdirSync(out,{recursive:true});
function capture(name){writeFileSync(`${out}/${name}.png`,run('exec-out','screencap','-p'));writeFileSync(`${out}/${name}.xml`,xml());}
async function tapAt(position,long=false) {
 const [x,y,r,b]=bounds(node(await until(t=>node(t))));
 const [px,py]=position(x,y,r,b).map(Math.round);
 if(long)shell('input','swipe',String(px),String(py),String(px),String(py),'850');
 else shell('input','tap',String(px),String(py));
 await delay(700);
}
async function expect(value){await until(t=>node(t)?.includes(`text="${value}"`));}
shell('am','force-stop',pkg);shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);
await tapAt((x,y,r,b)=>[(x+r)/2,(y+b)/2]);
shell('input','text','hello%sworld');await expect('hello world');
await tapAt((x,y,r,b)=>[x+33,(y+b)/2]);shell('input','text','X');await expect('Xhello world');
await tapAt((x,y,r,b)=>[r-35,(y+b)/2]);shell('input','text','Z');await expect('Xhello worldZ');
await tapAt((x,y,r,b)=>[x+85,(y+b)/2],true);capture('word-selection');
shell('input','text','hi');await expect('hi worldZ');capture('word-replaced');
// Moving before the timeout cancels long-press selection.
const [x,y,r,b]=bounds(node(xml()));
shell('input','swipe',String(x+70),String(Math.round((y+b)/2)),String(x+240),String(Math.round((y+b)/2)),'250');
await delay(650);shell('input','text','Q');await expect('hiQ worldZ');
shell('input','keyevent','KEYCODE_ENTER');
await until(t=>node(t,'greeting-sent')?.includes('Saved: hiQ worldZ'));
capture('submitted');
// Multi-line hit testing chooses a visual line and its logical insertion point.
const mode=bounds(node(xml(),'note-mode'));
shell('input','tap',String(Math.round((mode[0]+mode[2])/2)),String(Math.round((mode[1]+mode[3])/2)));
await tapAt((x,y,r,b)=>[r-35,b-35]);shortcut('A');shell('input','text','line0');
shell('input','keyevent','KEYCODE_ENTER');shell('input','text','line1');
shell('input','keyevent','KEYCODE_ENTER');shell('input','text','line2');
await until(t=>node(t)?.includes('line2'));
await tapAt((x,y,r,b)=>[x+33,y+45]);shell('input','text','A');
await until(t=>node(t)?.includes('text="Aline0&#10;line1&#10;line2"'));
capture('multiline-first');
const pid=shell('pidof',pkg).trim(),log=run('logcat','-d',`--pid=${pid}`).toString();
assert.doesNotMatch(log,/FATAL EXCEPTION|UnsatisfiedLinkError|Render failed/);
writeFileSync(`${out}/device.log`,log);
console.log('PASS: GPU tap caret placement, long-press word replacement, moved-press cancellation, submit and multiline positioning on physical Android');
