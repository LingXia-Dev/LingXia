// Physical Pixel 3 / Gboard English QWERTY probe. Coordinates are normalized to its keyboard layout.
import {execFileSync} from 'node:child_process';
import {mkdirSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
const [adb,serial]=process.argv.slice(2);
const pkg='com.lingxia.example.gpu.dev', launcher=`${pkg}/com.lingxia.example.gpu.MainActivity`;
const run=(...args)=>execFileSync(adb,['-s',serial,...args],{timeout:30000});
const shell=(...args)=>run('shell',...args).toString();
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const xml=()=>{shell('uiautomator','dump','/sdcard/gpu-ime.xml');return shell('cat','/sdcard/gpu-ime.xml');};
const node=(tree,id)=>tree.match(new RegExp(`<node[^>]*resource-id="${pkg}:id/${id}"[^>]*>`))?.[0];
async function until(check) {
 for(let i=0;i<15;i++){const tree=xml();if(check(tree))return tree;await delay(300);}
 throw Error('GPU input state timed out');
}
async function tapInput(){
 const tree=await until(t=>node(t,'greeting-input'));
 const target=node(tree,'greeting-input'); assert.match(target,/clickable="true"/);
 const [x,y,r,b]=target.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
 shell('input','tap',String(r-15),String(Math.round((y+b)/2)));
 await delay(800);
 assert.match(shell('dumpsys','input_method'),/mInputShown=true/);
}
const output='target/gpu-ime-evidence'; mkdirSync(output,{recursive:true});
function capture(name){writeFileSync(`${output}/${name}.png`,run('exec-out','screencap','-p'));writeFileSync(`${output}/${name}.xml`,xml());}
async function captureBlink(name){
 for(let i=0;i<6;i++){writeFileSync(`${output}/${name}-${i}.png`,run('exec-out','screencap','-p'));await delay(130);}
 writeFileSync(`${output}/${name}.xml`,xml());
}
shell('am','force-stop',pkg);shell('am','start','-n',launcher);
const theme=node(await until(t=>node(t,'theme')),'theme');
if(theme.includes('text="Light"')){
 const [x,y,r,b]=theme.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
 shell('input','tap',String((x+r)/2),String((y+b)/2));
 await until(t=>node(t,'theme')?.includes('text="Dark"'));
}
await tapInput();
const png=run('exec-out','screencap','-p'),width=png.readUInt32BE(16),height=png.readUInt32BE(20);
function key(x,y){shell('input','tap',String(Math.round(width*x)),String(Math.round(height*y)));}
for(const [x,y] of [[.5963,.7375],[.25,.6639],[.8935,.7375],[.8935,.7375],[.8444,.6639]])key(x,y);
await until(tree=>node(tree,'greeting-input')?.includes('text="hello"'));
assert.match(node(xml(),'greeting-preview'),/Make it personal/);
capture('preedit');
key(.9194,.8986);
await until(tree=>node(tree,'greeting-sent')?.includes('Saved: hello'));
assert.match(node(xml(),'greeting-preview'),/Your note: hello/);
await delay(500);assert.match(shell('dumpsys','input_method'),/mInputShown=false/);
capture('submitted');
shell('input','keyevent','KEYCODE_HOME');await delay(500);shell('am','start','-n',launcher);
await until(tree=>node(tree,'greeting-sent')?.includes('Saved: hello'));
await tapInput();key(.9194,.8204);key(.9194,.8986);
await until(tree=>node(tree,'greeting-sent')?.includes('text="Saved: hell"'));
capture('resumed-delete');
await tapInput(); key(.8444,.6639);
await until(tree=>node(tree,'greeting-input')?.includes('text="hello"'));
assert.match(node(xml(),'greeting-preview'),/Your note: hell/);
shell('input','keyevent','KEYCODE_HOME');await delay(500);shell('am','start','-n',launcher);
await until(tree=>node(tree,'greeting-input')?.includes('text="hell"'));
capture('cancelled-preedit');
await tapInput();
shell('input','keyevent',...Array(4).fill('KEYCODE_DEL'));
const longText='0123456789'.repeat(8);
for(let i=0;i<8;i++){shell('input','text','0123456789');await delay(200);}
await until(tree=>node(tree,'greeting-input')?.includes(`text="${longText}"`));
await captureBlink('long-input-end');
for(let i=0;i<7;i++){shell('input','keyevent',...Array(10).fill('KEYCODE_DEL'));await delay(200);}
shell('input','keyevent',...Array(6).fill('KEYCODE_DEL'));
await until(tree=>node(tree,'greeting-input')?.includes('text="0123"'));
await captureBlink('shortened-input');
key(.9194,.8986);
const mode=node(await until(t=>node(t,'note-mode')),'note-mode');
const [mx,my,mr,mb]=mode.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
shell('input','tap',String(Math.round((mx+mr)/2)),String(Math.round((my+mb)/2)));
await tapInput();shell('input','keyevent',...Array(4).fill('KEYCODE_DEL'));
for(let i=0;i<10;i++){if(i)shell('input','keyevent','KEYCODE_ENTER');shell('input','text',`line-0${i}`);await delay(200);}
await until(tree=>node(tree,'greeting-input')?.includes('line-09'));
await captureBlink('multiline-end');
for(let i=0;i<7;i++){shell('input','keyevent',...Array(10).fill('KEYCODE_DEL'));await delay(200);}
shell('input','keyevent',...Array(5).fill('KEYCODE_DEL'));
await until(tree=>node(tree,'greeting-input')?.includes('text="line"'));
await captureBlink('multiline-short');
const pid=shell('pidof',pkg).trim(), logs=run('logcat','-d',`--pid=${pid}`).toString();
assert.doesNotMatch(logs,/FATAL EXCEPTION|UnsatisfiedLinkError|Render failed/);
writeFileSync(`${output}/device.log`,logs);
const presents=()=>run('logcat','-d',`--pid=${pid}`).toString().split('\n').filter(line=>line.includes('present generation='));
const before=presents().length;await delay(2200);const blinking=presents().slice(before);
assert(blinking.length>=3 && blinking.length<=7,`expected phase frames, got ${blinking.length}`);
assert(blinking.every(line=>line.includes('rebuild=false')));
assert(blinking.some(line=>line.includes('caretVisible=false')) && blinking.some(line=>line.includes('caretVisible=true')));
shell('input','keyevent','KEYCODE_HOME');await delay(700);
const paused=presents().length;await delay(1500);assert.equal(presents().length,paused,'background page must stop blinking');
shell('am','start','-n',launcher);await delay(1000);
const unfocused=presents().length;await delay(1500);assert.equal(presents().length,unfocused,'unfocused page must be idle');
writeFileSync(`${output}/device.log`,run('logcat','-d',`--pid=${pid}`));
console.log('PASS: composition, submit, background recovery, single/multiline editing, phase-only blink frames and unfocused/background idle');
