import { test } from 'node:test';
import assert from 'node:assert/strict';
import { configureActions, createView, takeActionRequests, takeActionErrors, receiveActionResponse, reset, describe, h } from '../src/runtime.js';
const catalog = {__names:['ticker'],__modes:{ticker:'call'}};
function start() {
  configureActions(catalog);
  const view=createView({},{});
  const promise=view.channels.open('ticker',{symbol:'AAPL'});
  const [request]=takeActionRequests().map(JSON.parse);
  const frame=(kind,fields={})=>receiveActionResponse({v:2,kind,id:request.id,...fields});
  return {view,promise,request,frame};
}
test('channel establishment replays initial data and preserves independent sequences',async()=>{
  const {promise,request,frame}=start();
  assert.deepEqual(request,{v:2,kind:'ch.open',id:request.id,topic:'ticker',cap:'page',params:{symbol:'AAPL'}});
  frame('ch.ack',{ok:true});frame('ch.data',{seq:0,payload:{type:'init'}});
  const channel=await promise,seen=[];
  channel.on('data',v=>seen.push(v));
  assert.deepEqual(seen,[{type:'init'}]);assert.equal(Object.isFrozen(seen[0]),true);
  channel.send({symbol:'MSFT'});channel.send({symbol:'TSLA'});
  assert.deepEqual(takeActionRequests().map(JSON.parse).map(x=>[x.kind,x.seq,x.payload.symbol]),[['ch.data',0,'MSFT'],['ch.data',1,'TSLA']]);
  frame('ch.data',{seq:2,payload:{type:'tick'}});
  assert.equal(seen.length,2);channel.close('done','finished');
  assert.equal(takeActionRequests().map(JSON.parse)[0].kind,'ch.close');
  assert.equal(frame('ch.data',{seq:3,payload:'late'}),false);
  assert.throws(()=>channel.send({}),{code:'STREAM_CLOSED'});
});
test('malformed frames and duplicate ack leave the valid channel usable',async()=>{
  const {promise,frame}=start();
  assert.throws(()=>frame('ch.data',{seq:0,payload:1}),/before acknowledgement/);
  assert.throws(()=>frame('ch.ack',{ok:'yes'}),/invalid frame/);
  frame('ch.ack',{ok:true});const channel=await promise;
  assert.throws(()=>frame('ch.ack',{ok:true}),/duplicate/);
  assert.throws(()=>frame('ch.data',{seq:1,payload:1}),/sequence/);
  assert.throws(()=>frame('ch.data',{seq:0,payload:NaN}),/JSON/);
  const reader=channel[Symbol.asyncIterator](),next=reader.next();
  frame('ch.data',{seq:0,payload:'valid'});assert.deepEqual(await next,{done:false,value:'valid'});
  let closes=0;channel.on('close',()=>closes++);
  frame('ch.close',{code:'done'});assert.equal(closes,1);channel.close();assert.equal(closes,1);
  assert.deepEqual(await reader.next(),{done:true,value:undefined});
});
test('rejected opens and resets settle pending promises and invalidate old handles',async()=>{
  let {promise,frame,view}=start();
  frame('ch.ack',{ok:false,error:{code:'BRIDGE_TOPIC_NOT_FOUND',message:'missing'}});
  await assert.rejects(promise,{code:'BRIDGE_TOPIC_NOT_FOUND'});
  const old=view.channels;
  ({promise}=start());reset();await assert.rejects(promise,{code:'BRIDGE_CANCELED'});
  assert.throws(()=>old.open('ticker'),{code:'BRIDGE_CANCELED'});
  ({promise,frame}=start());frame('ch.ack',{ok:true});const channel=await promise;
  const reader=channel[Symbol.asyncIterator](),next=reader.next();reset();
  await assert.rejects(next,{code:'BRIDGE_CANCELED'});
  assert.throws(()=>channel.send({}),{code:'STREAM_CLOSED'});
});
test('channel creation/send/close are forbidden during render and topics stay page-scoped',async()=>{
  const {view,promise,frame}=start();
  assert.throws(()=>view.channels.open('host.terminal'),/page channel/);
  assert.throws(()=>view.channels.open('missing'),/page channel/);
  assert.throws(()=>describe(()=>{view.channels.open('ticker');return h('text',{},'bad')},{}),/during render/);
  frame('ch.ack',{ok:true});const channel=await promise;
  for(const effect of [()=>channel.send({}),()=>channel.close()])
    assert.throws(()=>describe(()=>{effect();return h('text',{},'bad')},{}),/during render/);
  channel.close();
});
test('initial-message and iterator backlogs are bounded and close the producer',async()=>{
  for(const iterate of [false,true]){
    const {promise,frame}=start();frame('ch.ack',{ok:true});const channel=await promise;
    const reader=iterate?channel[Symbol.asyncIterator]():null;let cause;
    channel.on('error',e=>cause=e);
    for(let i=0;i<257;i++)frame('ch.data',{seq:i,payload:i});
    assert.equal(cause.code,'UI_LIMIT');
    assert.equal(takeActionRequests().map(JSON.parse)[0].kind,'ch.close');
    if(reader)await assert.rejects(reader.next(),{code:'UI_LIMIT'});
  }
});
test('a failed send does not consume sequence and iterator return closes once',async()=>{
  const {promise,frame}=start();frame('ch.ack',{ok:true});const channel=await promise;
  assert.throws(()=>channel.send('x'.repeat(70000)),/payload too large/);
  assert.throws(()=>channel.send(NaN),/JSON/);
  channel.send('valid');assert.equal(JSON.parse(takeActionRequests()[0]).seq,0);
  const reader=channel[Symbol.asyncIterator]();await reader.return();await reader.return();
  assert.equal(takeActionRequests().length,1);
});
test('remote close preserves already accepted iterator messages before EOF',async()=>{
  const {promise,frame}=start();frame('ch.ack',{ok:true});const channel=await promise;
  const reader=channel[Symbol.asyncIterator]();
  frame('ch.data',{seq:0,payload:'last'});frame('ch.close',{code:'done'});
  assert.deepEqual(await reader.next(),{done:false,value:'last'});
  assert.deepEqual(await reader.next(),{done:true,value:undefined});
});
test('admission reserves enough close frames even when the data outbox is full',async()=>{
  configureActions(catalog);const view=createView({},{});
  const pending=Array.from({length:256},()=>view.channels.open('ticker'));
  const opens=takeActionRequests().map(JSON.parse);
  assert.throws(()=>view.channels.open('ticker'),/too many pending/);
  assert.throws(()=>view.actions.ticker({}),/too many pending/);
  for(const request of opens)receiveActionResponse({v:2,kind:'ch.ack',id:request.id,ok:true});
  const handles=await Promise.all(pending);
  for(const channel of handles)channel.send('data');
  assert.throws(()=>handles[0].send('overflow'),/too many pending/);
  for(const channel of handles)channel.close();
  const frames=takeActionRequests().map(JSON.parse);
  assert.equal(frames.filter(x=>x.kind==='ch.data').length,256);
  assert.equal(frames.filter(x=>x.kind==='ch.close').length,256);
  const reopened=view.channels.open('ticker');reset();
  await assert.rejects(reopened,{code:'BRIDGE_CANCELED'});
});
test('byte limits close a slow consumer and listener failures do not stop delivery',async()=>{
  let {promise,frame}=start();frame('ch.ack',{ok:true});
  let channel=await promise,seen=[];
  channel.on('data',()=>{throw Error('listener failed')});
  channel.on('data',value=>seen.push(value));
  frame('ch.data',{seq:0,payload:'first'});
  assert.deepEqual(seen,['first']);assert.match(takeActionErrors()[0],/listener failed/);
  channel.close();takeActionRequests();
  ({promise,frame}=start());frame('ch.ack',{ok:true});channel=await promise;
  const reader=channel[Symbol.asyncIterator]();
  for(let i=0;i<9;i++)frame('ch.data',{seq:i,payload:'x'.repeat(65530)});
  await assert.rejects(reader.next(),{code:'UI_LIMIT'});
  assert.equal(JSON.parse(takeActionRequests()[0]).kind,'ch.close');
});
