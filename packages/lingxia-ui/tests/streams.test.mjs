import { test } from 'node:test';
import assert from 'node:assert/strict';
import { configureActions, createView, takeActionRequests, receiveActionResponse, reset, takeActionErrors } from '../src/runtime.js';

function start() {
  configureActions({__names:['watch'], __modes:{watch:'stream'}});
  const stream = createView({}, {}).actions.watch({count:3});
  const [request] = takeActionRequests().map(JSON.parse);
  return {stream, request, event: (seq, payload) => receiveActionResponse({v:2,kind:'event',id:request.id,seq,payload})};
}
test('stream events, terminal result and listener isolation', async () => {
  const {stream, request, event} = start(), seen=[];
  assert.equal(request.method,'watch');
  stream.on('data', () => {throw Error('listener');}).on('data', v => seen.push(v));
  stream.on('end', v=>seen.push(v));
  event(0,{token:'one'}); event(1,{token:'two'});
  assert.deepEqual(seen,[{token:'one'},{token:'two'}]);
  assert.equal(Object.isFrozen(seen[0]),true);
  assert.deepEqual(takeActionErrors(),['Error: listener','Error: listener']);
  receiveActionResponse({v:2,kind:'res',id:request.id,ok:true,result:'done'});
  assert.equal(await stream.result,'done');
  assert.equal(event(2,'late'),false);
  stream.cancel(); assert.deepEqual(takeActionRequests(),[]);
  assert.equal(seen.at(-1),'done');
});
test('stream cancellation is terminal locally and emits one cancel frame', async () => {
  const {stream, request, event}=start();
  stream.cancel(); stream.cancel();
  await assert.rejects(stream.result,{code:'BRIDGE_CANCELED'});
  assert.deepEqual(takeActionRequests().map(JSON.parse),[{v:2,kind:'cancel',id:request.id}]);
  assert.equal(event(0,'late'),false);
  assert.equal(receiveActionResponse({v:2,kind:'res',id:request.id,ok:false,error:{code:'BRIDGE_CANCELED'}}),false);
});
test('iterator preserves ordered payloads and final result; early return cancels', async () => {
  let {stream,request,event}=start();
  const iterator=stream[Symbol.asyncIterator]();
  const waiting=iterator.next();event(0,'a');event(1,'b');
  receiveActionResponse({v:2,kind:'res',id:request.id,ok:true,result:'final'});
  assert.deepEqual(await waiting,{done:false,value:'a'});
  assert.deepEqual(await iterator.next(),{done:false,value:'b'});
  assert.deepEqual(await iterator.next(),{done:true,value:'final'});
  ({stream}=start());const next=stream[Symbol.asyncIterator]();
  await next.return();await assert.rejects(stream.result,{code:'BRIDGE_CANCELED'});
});
test('malformed ordering cannot consume the expected event; reset rejects pending reads', async () => {
  const {stream,event}=start(), iterator=stream[Symbol.asyncIterator]();
  assert.throws(()=>event(1,'bad'),/sequence/);
  assert.throws(()=>event(-1,'bad'),/invalid response/);
  assert.throws(()=>event(0,NaN),/JSON/);
  event(0,'good');assert.deepEqual(await iterator.next(),{done:false,value:'good'});
  assert.throws(()=>event(0,'duplicate'),/sequence/);
  event(2,'monotonic');assert.deepEqual(await iterator.next(),{done:false,value:'monotonic'});
  const pending=iterator.next();reset();
  await assert.rejects(pending,{code:'BRIDGE_CANCELED'});
  await assert.rejects(stream.result,{code:'BRIDGE_CANCELED'});
});
test('slow consumers are bounded while event-only listeners do not accumulate a queue', async () => {
  let {stream,event}=start();stream[Symbol.asyncIterator]();
  for(let n=0;n<=256;n++)event(n,n);
  await assert.rejects(stream.result,{code:'UI_LIMIT'});
  assert.equal(takeActionRequests().length,1);
  ({stream,event}=start());let count=0;stream.on('data',()=>count++);
  for(let n=0;n<1000;n++)event(n,n);
  assert.equal(count,1000);stream.cancel();await assert.rejects(stream.result);
});
test('terminal bridge failure rejects readers and reports the original structured error', async () => {
  const {stream,request}=start(), reader=stream[Symbol.asyncIterator]();
  const next=reader.next(), errors=[];stream.on('error',error=>errors.push(error));
  receiveActionResponse({v:2,kind:'res',id:request.id,ok:false,error:{code:'BRIDGE_INTERNAL_ERROR',message:'failed',data:{stage:'generator'}}});
  await assert.rejects(next,{code:'BRIDGE_INTERNAL_ERROR',message:'failed'});
  await assert.rejects(stream.result,{data:{stage:'generator'}});
  assert.equal(errors.length,1);
});
test('cancellation still has capacity when all request slots are occupied', async () => {
  configureActions({__names:['watch'],__modes:{watch:'stream'}});
  const actions=createView({},{}).actions;
  const streams=Array.from({length:256},()=>actions.watch());
  assert.throws(()=>actions.watch(),/too many pending/);
  for(const stream of streams)stream.cancel();
  const frames=takeActionRequests().map(JSON.parse);
  assert.equal(frames.filter(frame=>frame.kind==='req').length,256);
  assert.equal(frames.filter(frame=>frame.kind==='cancel').length,256);
  await Promise.all(streams.map(stream=>assert.rejects(stream.result,{code:'BRIDGE_CANCELED'})));
});
test('iterator payload bytes are bounded independently of the item count', async () => {
  const {stream,event}=start();stream[Symbol.asyncIterator]();
  for(let i=0;i<18;i++)event(i,'x'.repeat(32768));
  await assert.rejects(stream.result,{code:'UI_LIMIT'});
});
