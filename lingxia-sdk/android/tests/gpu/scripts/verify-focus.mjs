import {execFileSync} from 'node:child_process';
import {readFile,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const adb=process.env.ADB ?? 'adb',serial=process.argv[2] ?? process.env.ANDROID_SERIAL,pkg='com.lingxia.example.gpu.dev';
assert.ok(serial, 'Pass the device serial or set ANDROID_SERIAL.');
const file='examples/lingxia-showcase-gpu/lxapp/pages/home/index.ui.tsx',original=await readFile(file,'utf8');
const probe=`import {useLocal,type View} from '@lingxia/ui';
export function render({env}:View){const count=useLocal('count',0),trap=useLocal('trap',0);return <column style={{background:'#FFFFFF',padding:20,gap:12}}><spacer style={{height:env.insets.top}}/><text testId="focus-count" style={{color:'#000000'}}>{String(count.value)}</text><text testId="focus-trap" style={{color:'#000000'}}>{String(trap.value)}</text>{count.value>0&&<button key="other" testId="focus-other" onClick={()=>trap.set(trap.value+1)} style={{height:60,background:'#D03030'}}>Other action</button>}{count.value<3&&<button key="target" testId="focus-target" onClick={()=>count.set(count.value+1)} style={{height:70,background:'#2458DA',radius:14}}>{'Target '+count.value}</button>}</column>}
`;
const run=(...a)=>execFileSync(adb,['-s',serial,...a],{timeout:30000});
const shell=(...a)=>run('shell',...a).toString(),delay=ms=>new Promise(r=>setTimeout(r,ms));
const dump=()=>{shell('uiautomator','dump','/sdcard/gpu-focus.xml');return shell('cat','/sdcard/gpu-focus.xml');};
const node=(xml,id)=>xml.match(new RegExp(`<node[^>]*resource-id="${pkg}:id/${id}"[^>]*>`))?.[0];
async function until(check){for(let i=0;i<35;i++){const xml=dump();if(check(xml))return xml;await delay(500);}throw Error('Focus state timed out');}
const count=n=>until(x=>node(x,'focus-count')?.includes(`text="${n}"`));
const key=name=>shell('input','keyevent',name);
async function focusPixel(name){const xml=await until(x=>node(x,'focus-target'));const [l,t,r]=node(xml,'focus-target').match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);await delay(300);const path=`target/showcase-gpu-evidence/focus-${name}.png`;await writeFile(path,run('exec-out','screencap','-p'));const p=JSON.parse(execFileSync('python',['-c','from PIL import Image; import sys,json; print(json.dumps(Image.open(sys.argv[1]).convert("RGB").getpixel((int(sys.argv[2]),int(sys.argv[3])))))',path,String(Math.round((l+r)/2)),String(t+2)]).toString());assert.deepEqual(p,[23,32,51]);}
const rotation=shell('settings','get','system','user_rotation').trim(),auto=shell('settings','get','system','accelerometer_rotation').trim();
await writeFile('target/ui-focus-original.tsx',original);
try{
 await writeFile(file,probe);await count(0);shell('input','tap','10','500');key('KEYCODE_TAB');await focusPixel('initial');
 key('KEYCODE_ENTER');await count(1);await focusPixel('reordered');
 shell('settings','put','system','accelerometer_rotation','0');shell('settings','put','system','user_rotation','1');await until(x=>x.includes('rotation="1"'));await focusPixel('landscape');
 shell('settings','put','system','user_rotation',rotation);shell('settings','put','system','accelerometer_rotation',auto);await until(x=>x.includes(`rotation="${rotation}"`));
 key('KEYCODE_ENTER');await count(2);key('KEYCODE_ENTER');await count(3);
 key('KEYCODE_ENTER');await delay(500);assert.match(node(dump(),'focus-trap'),/text="0"/);
 key('KEYCODE_TAB');key('KEYCODE_ENTER');await until(x=>node(x,'focus-trap')?.includes('text="1"'));
 console.log('PASS: keyed focus after callback/label update, sibling insertion, rotation, removal without fallback activation');
}finally{
 shell('settings','put','system','user_rotation',rotation);shell('settings','put','system','accelerometer_rotation',auto);
 if(await readFile(file,'utf8')===probe){await writeFile(file,original);await until(x=>node(x,'tab-hello'));}
 else throw Error('Probe changed externally; original saved in target/ui-focus-original.tsx');
}
