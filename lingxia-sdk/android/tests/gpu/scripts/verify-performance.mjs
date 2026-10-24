import {execFileSync} from 'node:child_process';
import {mkdirSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
import { openPlayground } from './open-playground.mjs';
const [adb,serial,output='target/gpu-performance-evidence']=process.argv.slice(2),pkg='com.lingxia.example.gpu.dev';
const run=(...args)=>execFileSync(adb,['-s',serial,...args],{timeout:30000}).toString();
const shell=(...args)=>run('shell',...args).trim();
const delay=ms=>new Promise(r=>setTimeout(r,ms));
const tree=()=>{shell('uiautomator','dump','/sdcard/gpu-perf.xml');return shell('cat','/sdcard/gpu-perf.xml');};
const find=(xml,id)=>xml.match(new RegExp(`<node[^>]*resource-id="${pkg}:id/${id}"[^>]*>`))?.[0];
const center=(xml,id)=>{const n=find(xml,id);assert.ok(n,id);const [x,y,r,b]=n.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);return [String((x+r)/2),String((y+b)/2)];};
const tap=p=>shell('input','tap',...p);
shell('am','force-stop',pkg);shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);await delay(2000);
await openPlayground(shell, tree);
const pid=shell('pidof',pkg),out=output;mkdirSync(out,{recursive:true});
const log=()=>run('logcat','-d',`--pid=${pid}`);
function frames(text){
 const gpu=new Map(text.split('\n').filter(l=>l.includes('GPU_PRESENT surface=')).map(line=>{
  const f=Object.fromEntries([...line.matchAll(/(\w+)=(\d+)/g)].map(m=>[m[1],Number(m[2])]));
  return [`${f.surface}/${f.generation}`,{surfaceLockUs:f.lockUs,surfaceAcquireUs:f.acquireUs,surfaceEncodeUs:f.encodeUs,surfaceSubmitUs:f.submitUs,surfacePresentUs:f.presentUs}];
 }));
 return text.split('\n').filter(l=>l.includes('present generation=')&&l.includes('metrics=')).map(line=>{
  const fields=Object.fromEntries([...line.matchAll(/(\w+)=(\d+)/g)].map(m=>[m[1],Number(m[2])]));
  const metrics=JSON.parse(line.slice(line.indexOf('metrics=')+8));
  assert.equal(fields.generation,metrics.generation);
  assert.ok(metrics.textUs<=metrics.layoutUs);
  const viewParts=['viewEvalUs','viewTransferUs','viewDecodeUs'];
  assert.ok([...viewParts,'wireValues'].every(k=>Number.isFinite(metrics[k])&&metrics[k]>=0));
  assert.ok(viewParts.reduce((sum,k)=>sum+metrics[k],0)<=metrics.viewUs);
  if(metrics.viewPasses===0)assert.ok([...viewParts,'wireValues'].every(k=>metrics[k]===0));
  const prepare=metrics.prepare;assert.ok(prepare,'GPU preparation breakdown is required');
  assert.ok(Object.values(prepare).every(v=>Number.isFinite(v)&&v>=0));
  assert.ok(Object.values(prepare).reduce((sum,v)=>sum+v,0)<=metrics.buildUs);
  const key=`${fields.surface}/${fields.generation}`;
  assert.ok(gpu.has(key),'accepted GPU frame needs matching presentation timings');
  return {key, ...fields, ...metrics, ...gpu.get(key),prepareSceneUs:prepare.sceneUs,prepareImagesUs:prepare.imagesUs,prepareEncodeUs:prepare.encodeUs,prepareSubmitUs:prepare.submitUs};
 });
}
const stats=values=>{
 const sorted=[...values].sort((a,b)=>a-b);
 return sorted.length?{p50:sorted[Math.ceil(sorted.length*.5)-1],p95:sorted[Math.ceil(sorted.length*.95)-1],max:sorted.at(-1)}:null;
};
const report={device:{serial,model:shell('getprop','ro.product.model'),android:shell('getprop','ro.build.version.release'),density:shell('wm','density')},units:'CPU wall microseconds; textUs is included in layoutUs; workerMs is milliseconds; not GPU timestamps or display FPS',workloads:{}};
async function phase(name,work){
 await delay(800);const before=new Set(frames(log()).map(f=>f.key));
 await work();await delay(800);
 const measured=frames(log()).filter(f=>!before.has(f.key));
 const summary={samples:measured.length,viewFrames:measured.filter(f=>f.viewPasses>0).length,layoutFrames:measured.filter(f=>f.layoutPasses>0).length,sceneCacheHits:measured.filter(f=>f.sceneCacheHit===true).length};
 for(const field of ['totalUs','viewUs','viewEvalUs','viewTransferUs','viewDecodeUs','wireValues','layoutUs','textUs','composeUs','buildUs','prepareSceneUs','prepareImagesUs','prepareEncodeUs','prepareSubmitUs','workerQueueUs','uiWaitUs','uiUs','surfaceLockUs','surfaceAcquireUs','surfaceEncodeUs','surfaceSubmitUs','surfacePresentUs','nodes','paints'])summary[field]=stats(measured.map(f=>f[field]));
 report.workloads[name]=summary;writeFileSync(`${out}/${name}.json`,JSON.stringify(measured,null,2));
 console.log(name,JSON.stringify(summary));return measured;
}
writeFileSync(`${out}/memory-before.txt`,shell('dumpsys','meminfo',pkg));
writeFileSync(`${out}/thermal-before.txt`,shell('dumpsys','thermalservice'));
const hello=center(tree(),'say-hello');
const counters=await phase('counter',async()=>{for(let i=0;i<20;i++){tap(hello);await delay(150);}});
assert.ok(counters.length>=20);assert.ok(counters.some(f=>f.viewPasses>0));
assert.ok(counters.some(f=>f.sceneCacheHit===true),'unchanged action frames should reuse scenes');
assert.ok(find(tree(),'hello-count')?.includes('20 times'));
tap(center(tree(),'greeting-input'));await delay(900);
const input=await phase('input',async()=>{for(let i=0;i<8;i++){shell('input','text','0123456789');await delay(150);}});
assert.ok(input.some(f=>f.layoutPasses>0));
const blink=await phase('focused-idle',()=>delay(2500));
assert.ok(blink.length>=3&&blink.length<=9);assert.ok(blink.every(f=>f.viewPasses===0&&f.layoutPasses===0));
assert.ok(blink.every(f=>f.sceneCacheHit===true),'warmed caret phases should reuse scenes');
shell('input','keyevent','KEYCODE_BACK');await delay(400);
tap(center(tree(),'tab-discover'));await delay(1200);
const listTree=tree(),first=center(listTree,'discover-first'),last=center(listTree,'discover-last');
const lists=await phase('list-window',async()=>{for(let i=0;i<20;i++){tap(i%2?first:last);await delay(300);}});
assert.ok(lists.length>=20);assert.ok(lists.some(f=>f.viewPasses>1));
shell('input','keyevent','KEYCODE_HOME');await delay(1000);
const background=await phase('background-idle',()=>delay(2500));assert.equal(background.length,0);
shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);await delay(1000);
writeFileSync(`${out}/memory-after.txt`,shell('dumpsys','meminfo',pkg));
writeFileSync(`${out}/thermal-after.txt`,shell('dumpsys','thermalservice'));
const logs=log();assert.doesNotMatch(logs,/FATAL EXCEPTION|UnsatisfiedLinkError|Render failed/);
writeFileSync(`${out}/device.log`,logs);writeFileSync(`${out}/report.json`,JSON.stringify(report,null,2));
console.log('PASS: measured counter, input, list windows, retained blink and background idle on physical Android');
