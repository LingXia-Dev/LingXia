import {execFileSync} from 'node:child_process';
import {mkdir,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const [adb,serial,mode]=process.argv.slice(2),pkg='com.lingxia.example.gpu.dev';
if(!adb||!serial||(mode&&mode!=='--current-page'))throw Error('Usage: node verify-images.mjs <adb> <serial> [--current-page]');
const run=(...a)=>execFileSync(adb,['-s',serial,...a],{timeout:30000});
const shell=(...a)=>run('shell',...a).toString();
const delay=ms=>new Promise(r=>setTimeout(r,ms));
const dump=()=>{shell('uiautomator','dump','/sdcard/gpu-images.xml');return shell('cat','/sdcard/gpu-images.xml');};
const node=(xml,id)=>xml.match(new RegExp(`<node[^>]*resource-id="${pkg}:id/${id}"[^>]*>`))?.[0];
const bounds=n=>n.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
async function until(check){for(let i=0;i<20;i++){const xml=dump();if(check(xml))return xml;await delay(300);}throw Error('Image state timed out');}
async function tap(id){const b=bounds(node(await until(x=>node(x,id)),id));shell('input','tap',String(Math.round((b[0]+b[2])/2)),String(Math.round((b[1]+b[3])/2)));}
if(mode!=='--current-page') { shell('am','force-stop',pkg);shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`); }
await mkdir('target/showcase-gpu-evidence',{recursive:true});
const pid=shell('pidof',pkg).trim(),rawLog=()=>run('logcat','-d',`--pid=${pid}`).toString();
// Home legitimately loads two resources before the single-source list workload.
const baseline=rawLog().trimEnd().split('\n').at(-1);
const log=()=>{const text=rawLog(),at=baseline?text.lastIndexOf(baseline):-1;return at<0?text:text.slice(at+baseline.length);};
await tap('tab-discover');
const lastImages=()=>Number([...log().matchAll(/present generation=.* images=(\d+)/g)].at(-1)?.[1]??-1);
async function image(id,file){
 const n=node(await until(x=>node(x,id)&&lastImages()===1),id);assert.ok(n.includes('android.widget.ImageView'));
 const b=bounds(n);await writeFile(file,run('exec-out','screencap','-p'));
 const count=Number(execFileSync('python',['-c',`from PIL import Image; import sys; im=Image.open(sys.argv[1]).convert('RGB'); b=list(map(int,sys.argv[2:])); print(sum(1 for r,g,v in im.crop(tuple(b)).getdata() if r<60 and g>100 and v>50))`,file,...b.map(String)]).toString());
 assert.ok(count>30,`Raster logo missing from ${id}: ${count} green pixels`);
}
await image('discovery-image-1','target/showcase-gpu-evidence/image-first.png');
await tap('discover-last');await image('discovery-image-1000','target/showcase-gpu-evidence/image-last.png');
await tap('tab-hello');await until(()=>lastImages()===0);
await tap('tab-discover');await tap('discover-first');await image('discovery-image-1','target/showcase-gpu-evidence/image-reloaded.png');
const resumed=shell('dumpsys','activity','activities').split('\n').find(line=>line.includes('mResumedActivity:')&&line.includes(pkg+'/'));
const taskId=resumed?.match(/\bt(\d+)\}/)?.[1];assert.ok(taskId,'GPU task must be foreground before suspension');
shell('input','keyevent','KEYCODE_HOME');await delay(800);shell('am','task','focus',taskId);
await image('discovery-image-1','target/showcase-gpu-evidence/image-resume.png');
await delay(2000);const before=(log().match(/present generation=/g)||[]).length;await delay(5000);
assert.equal((log().match(/present generation=/g)||[]).length,before,'Image loading never settled');
const logs=log();await writeFile('target/showcase-gpu-evidence/image-host.log',logs);
assert.doesNotMatch(logs,/UI_IMAGE:|Image failed:|GPU image resource failed|Render failed|FATAL EXCEPTION/);
assert.ok([...logs.matchAll(/present generation=.* images=(\d+)/g)].every(m=>Number(m[1])<=1),'Repeated rows uploaded duplicate images');
console.log('PASS: GPU image pixels, rounded image nodes, first/end rows, eviction/reload, surface resume, shared source, idle');
