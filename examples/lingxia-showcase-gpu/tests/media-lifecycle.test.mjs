import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
const require = createRequire(new URL('../../../packages/lingxia-ui/package.json', import.meta.url));
const source = require('esbuild').buildSync({ entryPoints: [fileURLToPath(new URL('../lxapp/pages/media/index.ts', import.meta.url))], bundle: true, write: false, format: 'iife' }).outputFiles[0].text;
function deferred() { let resolve, reject; const promise = new Promise((a,b) => { resolve=a; reject=b; }); return {promise,resolve,reject}; }
function task() {
 const end = deferred(), result = deferred();
 return { end, result: result.promise, finish: result, cancels: 0, cancel() {this.cancels++;}, progress: { [Symbol.asyncIterator]() {return this;}, next() {return end.promise;} } };
}
function fixture() {
 let page, disposed = false, late = 0;
 const tasks = [], toasts = [], image = deferred();
 const lx = { navigationBar: {update: async () => {}}, compressVideo() {const t=task();tasks.push(t);return t;}, compressImage: () => image.promise, showToast: value => toasts.push(value) };
 vm.runInNewContext(source, { lx, console, AbortController, Page(definition) {
  page={...definition,data:structuredClone(definition.data),setData(patch){if(disposed)late++;Object.assign(this.data,patch);}};
 }});
 page.data.thumbnailVideoPath='lx://temp/video';
 return {page,tasks,lx,image,toasts,late:()=>late,unload(){disposed=true;page.onUnload();}};
}
test('unload cancels video work and discards late completion without writing or toast', async () => {
 const f=fixture(), run=f.page.compressSelectedVideo(), t=f.tasks[0];f.unload();
 assert.equal(t.cancels,1);t.end.resolve({done:true});t.finish.resolve({uri:'old'});await run;
 assert.equal(f.late(),0);assert.equal(f.toasts.length,0);assert.equal(f.page._compressVideoTask,null);
});
test('cancel then restart retains the new task when the old result settles', async () => {
 const f=fixture(), first=f.page.compressSelectedVideo(), old=f.tasks[0];f.page.cancelVideoCompress();
 assert.equal(old.cancels,1);assert.equal(f.page.data.videoCompressBusy,false);
 const second=f.page.compressSelectedVideo(), current=f.tasks[1];
 old.end.resolve({done:true});old.finish.reject(Error('late cancellation'));await first;
 assert.equal(f.page._compressVideoTask,current);assert.equal(f.page.data.videoCompressBusy,true);
 current.end.resolve({done:true});current.finish.resolve({uri:'new'});await second;
 assert.equal(f.page.data.videoCompressResult.uri,'new');assert.equal(f.page.data.videoCompressBusy,false);
});
test('synchronous task creation failure restores controls and surfaces the error', async () => {
 const f=fixture();f.lx.compressVideo=()=>{throw Error('unsupported codec');};await f.page.compressSelectedVideo();
 assert.equal(f.page.data.videoCompressBusy,false);assert.match(f.page.data.videoCompressError,/unsupported codec/);
});
test('hidden image compression cannot publish late output', async () => {
 const f=fixture();f.page.data.imageInfoResult={path:'lx://temp/image'};
 const run=f.page.compressSelectedImage();f.page.onHide();f.image.resolve({uri:'late'});await run;
 assert.equal(f.page.data.compressing,false);assert.equal(f.page.data.compressResult,null);assert.equal(f.toasts.length,0);
});
test('preview unload aborts its handle and releases the change listener on rejection', async () => {
 const f=fixture(), completed=deferred();let signal, removed=0;
 f.lx.previewMedia=request=>{signal=request.signal;return {presented:Promise.resolve({status:'presented'}),completed:completed.promise,onChange(){return ()=>removed++;}};};
 const run=f.page._runPreviewSession({sources:['lx://temp/image']});f.unload();
 assert.equal(signal.aborted,true);completed.reject(Error('aborted'));await run;
 assert.equal(removed,1);assert.equal(f.late(),0);assert.equal(f.toasts.length,0);
});
