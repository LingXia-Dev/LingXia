import {test} from 'node:test';
import assert from 'node:assert/strict';
import {configureActions, createView, takeActionRequests, receiveActionResponse, reset, describe, h, dispatch, takeActionErrors} from '../src/runtime.js';
const catalog={__names:['increment'],__modes:{increment:'call'}};
test('actions preserve LegacyV2 payloads and settle responses exactly once', async()=>{
  configureActions(catalog);
  const {actions}=createView({},{}), payload={step:1};
  const promise=actions.increment(payload);payload.step=9;
  const frame=JSON.parse(takeActionRequests()[0]);
  assert.deepEqual(frame,{v:2,kind:'req',id:frame.id,method:'increment',cap:'page',params:{step:1}});
  assert.deepEqual(takeActionRequests(),[]);
  assert.equal(receiveActionResponse({v:2,kind:'res',id:frame.id,ok:true,result:{count:1}}),true);
  assert.deepEqual(await promise,{count:1});
  assert.equal(receiveActionResponse({v:2,kind:'res',id:frame.id,ok:true,result:2}),false);
  const failed=actions.increment();const req=JSON.parse(takeActionRequests()[0]);
  receiveActionResponse({v:2,kind:'res',id:req.id,ok:false,error:{code:'E_PERMISSION_DENIED',message:'denied',data:{route:'increment'}}});
  await assert.rejects(failed,{name:'LxBridgeError',code:'E_PERMISSION_DENIED',message:'denied',data:{route:'increment'}});
});
test('reset cancels pending work and stale closures cannot target the replacement',async()=>{
  configureActions(catalog);const old=createView({},{}).actions;
  const pending=old.increment();const frame=JSON.parse(takeActionRequests()[0]);
  reset();await assert.rejects(pending,{code:'BRIDGE_CANCELED'});
  configureActions(catalog);assert.throws(()=>old.increment(),/Stale/);
  const next=createView({},{}).actions.increment();const replacement=JSON.parse(takeActionRequests()[0]);
  assert.notEqual(frame.id,replacement.id);
  assert.equal(receiveActionResponse({v:2,kind:'res',id:frame.id,ok:true}),false);
  receiveActionResponse({v:2,kind:'res',id:replacement.id,ok:true});await next;
});
test('catalog and render validation reject invalid modes, payloads and side effects',()=>{
  configureActions(catalog);const {actions}=createView({},{});
  assert.throws(()=>configureActions({__names:['watch'],__modes:{watch:'channel'}}),/catalog/);
  assert.throws(()=>actions.increment(()=>{}),/JSON/);
  assert.throws(()=>actions.increment({n:NaN}),/JSON/);
  assert.throws(()=>actions.increment(1,2),/at most one/);
  assert.throws(()=>actions.increment('x'.repeat(70000)),/payload too large/);
  assert.throws(()=>describe(()=>{actions.increment();return h('text',{},'bad');},{}),/during render/);
  assert.deepEqual(takeActionRequests(),[]);reset();
});
test('async event failures are reported without poisoning subsequent events',async()=>{
  configureActions(catalog);
  const buffer=describe(()=>h('button',{onClick:async()=>{throw Error('failed click');}},'go'),{});
  const handle=buffer[7];assert.equal(dispatch(handle),true);
  await Promise.resolve();await Promise.resolve();
  assert.deepEqual(takeActionErrors(),['Error: failed click']);
  assert.equal(dispatch(handle),true);reset();await Promise.resolve();await Promise.resolve();
  assert.deepEqual(takeActionErrors(),[]);
});
test('pending action limits and malformed replies cannot lose a live request',async()=>{
  configureActions(catalog);const {actions}=createView({},{});
  const promises=Array.from({length:256},()=>actions.increment());
  assert.throws(()=>actions.increment(),/too many pending/);
  const frames=takeActionRequests().map(JSON.parse);
  assert.throws(()=>receiveActionResponse({v:3,kind:'res',id:frames[0].id,ok:true}),/invalid response/);
  assert.throws(()=>receiveActionResponse({v:2,kind:'res',id:frames[0].id,ok:true,result:NaN}),/JSON/);
  for(const frame of frames) receiveActionResponse({v:2,kind:'res',id:frame.id,ok:true,result:42});
  assert.deepEqual(await Promise.all(promises),Array(256).fill(42));
  reset();
});
