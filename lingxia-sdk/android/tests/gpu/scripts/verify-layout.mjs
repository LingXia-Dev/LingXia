import {execFileSync} from 'node:child_process';
import {mkdirSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
const [adb,serial]=process.argv.slice(2),pkg='com.lingxia.example.gpu.dev';
const run=(...args)=>execFileSync(adb,['-s',serial,...args],{timeout:30000});
const shell=(...args)=>run('shell',...args).toString().trim();
const delay=ms=>new Promise(r=>setTimeout(r,ms));
const output='target/gpu-layout-evidence';mkdirSync(output,{recursive:true});
const hierarchy=()=>{shell('uiautomator','dump','/sdcard/gpu-layout.xml');return shell('cat','/sdcard/gpu-layout.xml');};
const find=(xml,attribute,value)=>[...xml.matchAll(/<node[^>]*>/g)].map(m=>m[0]).find(n=>n.includes(`${attribute}="${value}"`));
const node=(xml,id)=>find(xml,'resource-id',`${pkg}:id/${id}`);
const bounds=n=>n.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
const density=Number(shell('wm','density').match(/(?:Override|Physical) density: (\d+)/g).at(-1).match(/(\d+)$/)[1])/160;
async function capture(name,landscape){
 for(let i=0;i<20;i++){
  const png=run('exec-out','screencap','-p'),width=png.readUInt32BE(16),height=png.readUInt32BE(20);
  if((width>height)!==landscape){await delay(300);continue;}
  const xml=hierarchy(),theme=node(xml,'theme'),label=find(xml,'text','LINGXIA / SHOWCASE'),title=find(xml,'text','Small moments.');
  if(!theme||!label||!title){await delay(300);continue;}
  const t=bounds(theme),l=bounds(label),h=bounds(title);
  assert.ok(Math.abs((t[1]+t[3])/2-(l[1]+h[3])/2)<=3,'header center alignment');
  assert.ok(t[3]-t[1]>=44*density-2,'minimum theme touch height');
  if(landscape)assert.ok(Math.abs(t[2]-l[0]-(720-36)*density)<=4,`wide content maxWidth: ${t[2]-l[0]}`);
  assert.ok(node(xml,'hello-count')?.includes('Hello received 1 time.'),'state survives rotation');
  writeFileSync(`${output}/${name}.png`,png);writeFileSync(`${output}/${name}.xml`,xml);
  console.log(`PASS ${name}: centered header, minimum touch height, bounded content and retained state`);return;
 }
 throw Error('layout orientation timed out');
}
const auto=shell('settings','get','system','accelerometer_rotation'),rotation=shell('settings','get','system','user_rotation');
try {
 shell('settings','put','system','accelerometer_rotation','0');shell('settings','put','system','user_rotation','0');
 shell('am','force-stop',pkg);shell('am','start','-n',`${pkg}/com.lingxia.example.gpu.MainActivity`);await delay(1500);
 const box=bounds(node(hierarchy(),'say-hello'));shell('input','tap',String((box[0]+box[2])/2),String((box[1]+box[3])/2));await delay(500);
 await capture('portrait',false);
 shell('settings','put','system','user_rotation','1');await delay(1000);await capture('landscape',true);
} finally {
 shell('settings','put','system','user_rotation',rotation);shell('settings','put','system','accelerometer_rotation',auto);
}
