import { test } from 'node:test';
import assert from 'node:assert/strict';
import { bundlePage, mountPage } from './page-harness.mjs';
const source = bundlePage('../lxapp/pages/media/index.ts');
function deferred() { let resolve, reject; const promise = new Promise((a,b) => { resolve=a; reject=b; }); return {promise,resolve,reject}; }
function task() {
 const end = deferred(), result = deferred();
 return { end, result: result.promise, finish: result, cancels: 0, cancel() {this.cancels++;}, progress: { [Symbol.asyncIterator]() {return this;}, next() {return end.promise;} } };
}
function fixture() {
 let page, disposed = false, late = 0;
 const tasks = [], toasts = [], image = deferred();
 const lx = { navigationBar: {update: async () => {}}, media: { compressVideo() {const t=task();tasks.push(t);return t;}, compressImage: () => image.promise }, ui: { toast: value => toasts.push(value) } };
 page = mountPage(source, { lx, console }, { onWrite() { if (disposed) late++; } });
 page.data.thumbnailVideoPath='lx://temp/video';
 return {page,tasks,lx,image,toasts,late:()=>late,unload(){disposed=true;page.onUnload();}};
}
test('unload cancels video work and discards late completion without writing or toast', async () => {
 const f=fixture(), run=f.page.compressSelectedVideo(), t=f.tasks[0];f.unload();
 assert.equal(t.cancels,1);t.end.resolve({done:true});t.finish.resolve({path:'old'});await run;
 assert.equal(f.late(),0);assert.equal(f.toasts.length,0);
});
test('cancel then restart retains the new task when the old result settles', async () => {
 const f=fixture(), first=f.page.compressSelectedVideo(), old=f.tasks[0];f.page.cancelVideoCompress();
 assert.equal(old.cancels,1);assert.equal(f.page.data.videoCompressBusy,false);
 const second=f.page.compressSelectedVideo(), current=f.tasks[1];
 old.end.resolve({done:true});old.finish.reject(Error('late cancellation'));await first;
 assert.equal(f.page.data.videoCompressBusy,true);
 current.end.resolve({done:true});current.finish.resolve({path:'new'});await second;
 assert.equal(f.page.data.videoCompressResult.path,'new');assert.equal(f.page.data.videoCompressBusy,false);
});
test('synchronous task creation failure restores controls and surfaces the error', async () => {
 const f=fixture();f.lx.media.compressVideo=()=>{throw Error('unsupported codec');};await f.page.compressSelectedVideo();
 assert.equal(f.page.data.videoCompressBusy,false);assert.match(f.page.data.videoCompressError,/unsupported codec/);
});
test('hidden image compression cannot publish late output', async () => {
 const f=fixture();f.page.data.imageInfoResult={path:'lx://temp/image'};
 const run=f.page.compressSelectedImage();f.page.onHide();f.image.resolve({path:'late'});await run;
 assert.equal(f.page.data.compressing,false);assert.equal(f.page.data.compressResult,null);assert.equal(f.toasts.length,0);
});
test('preview unload aborts its handle and releases the change listener on rejection', async () => {
 const f=fixture(), completed=deferred();let signal, removed=0;
 f.lx.media.preview=request=>{signal=request.signal;return {presented:Promise.resolve({status:'presented'}),completed:completed.promise,onChange(){return ()=>removed++;}};};
 f.page.data.thumbnailResult={path:'lx://temp/image'};const run=f.page.previewVideoThumbnail();f.unload();
 assert.equal(signal.aborted,true);completed.reject(Error('aborted'));await run;
 assert.equal(removed,1);assert.equal(f.late(),0);assert.equal(f.toasts.length,0);
});
