import {test} from 'node:test';
import assert from 'node:assert/strict';
import {configureActions, createView, takeActionRequests, receiveActionResponse, reset, describe, h, dispatch, takeActionErrors, takeHandlerFailure, setLoudFailures} from '../src/internal.js';
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
  const buffer=describe(()=>h('button',{'data-testid':'go',onClick:async()=>{throw Error('failed click');}},'go'),{});
  const handle=buffer[7];
  const flush=async()=>{for(let i=0;i<4;i++) await Promise.resolve();};
  assert.equal(dispatch(handle),true);assert.equal(dispatch(handle),true);await flush();
  // Once, as a sync throw would have failed the page once.
  assert.equal(takeHandlerFailure(),'UI_HANDLER: onClick of <button data-testid="go"> rejected: failed click');
  assert.equal(takeHandlerFailure(),'');assert.deepEqual(takeActionErrors(),[]);
  assert.equal(dispatch(handle),true);reset();await flush();
  assert.equal(takeHandlerFailure(),'');
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
test('an action rejection nobody observes reaches the page log once',async()=>{
  configureActions(catalog);const {actions}=createView({},{});
  const flush=async()=>{for(let i=0;i<4;i++) await Promise.resolve();};
  actions.increment();
  const caught=actions.increment();caught.catch(()=>{});
  const [ignored,observed]=takeActionRequests().map(JSON.parse);
  receiveActionResponse({v:2,kind:'res',id:ignored.id,ok:false,error:{code:'E_IO',message:'disk full'}});
  receiveActionResponse({v:2,kind:'res',id:observed.id,ok:false,error:{code:'E_IO',message:'seen'}});
  await flush();
  assert.deepEqual(takeActionErrors(),["Unhandled rejection of action 'increment': disk full"]);
  const pending=actions.increment();takeActionRequests();
  reset();await flush();
  assert.deepEqual(takeActionErrors(),[],'a reset cancels quietly');
  void pending;
});
test('an action payload follows JSON for undefined and names what is not JSON',()=>{
  configureActions({__names:['reserve','ticker'],__modes:{reserve:'call',ticker:'call'}});const {actions,channels}=createView({},{});
  actions.reserve({replaces:undefined,slot:{from:1,to:undefined},seats:[1,undefined,{note:undefined}]}).catch(()=>{});
  assert.deepEqual(JSON.parse(takeActionRequests()[0]).params,{slot:{from:1},seats:[1,null,{}]});
  actions.reserve(undefined).catch(()=>{});
  assert.equal(JSON.parse(takeActionRequests()[0]).params,undefined);
  assert.throws(()=>actions.reserve({slot:{at:new Date(0)}}),{message:'UI_ACTION: reserve payload field slot.at is not JSON (Date)'});
  assert.throws(()=>actions.reserve({seats:[1,()=>{}]}),{message:'UI_ACTION: reserve payload field seats[1] is not JSON (function)'});
  assert.throws(()=>actions.reserve({n:NaN}),{message:'UI_ACTION: reserve payload field n is not JSON (NaN)'});
  assert.throws(()=>actions.reserve(new Map()),{message:'UI_ACTION: reserve payload value is not JSON (Map)'});
  channels.open('ticker',{symbol:'MSFT',since:undefined}).catch?.(()=>{});
  assert.deepEqual(JSON.parse(takeActionRequests()[0]).params,{symbol:'MSFT'});
  assert.throws(()=>channels.open('ticker',{at:1n}),{message:'UI_CHANNEL: ticker payload field at is not JSON (bigint)'});
  assert.deepEqual(takeActionRequests(),[]);reset();
});
test('quiet, a failed handler is logged with its site and the page carries on',async()=>{
  configureActions(catalog);setLoudFailures(false);
  const flush=async()=>{for(let i=0;i<4;i++) await Promise.resolve();};
  try {
    const later=describe(()=>h('button',{'data-testid':'go',onClick:async()=>{throw Error('offline');}},'go'),{});
    assert.equal(dispatch(later[7]),true);await flush();
    assert.equal(takeHandlerFailure(),'');
    assert.deepEqual(takeActionErrors(),['UI_HANDLER: onClick of <button data-testid="go"> rejected: offline']);
    const now=describe(()=>h('button',{'data-testid':'go',onClick:()=>{throw Error('offline');}},'go'),{});
    assert.equal(dispatch(now[7]),true);
    const [logged]=takeActionErrors();
    assert.ok(logged.startsWith('UI_HANDLER: onClick of <button data-testid="go"> threw: offline'),logged);
  } finally { setLoudFailures(true);reset(); }
});

test('an action the catalog does not hold names itself and the page; probes and symbols read as absent', async()=>{
  const {setActionPage}=await import('../src/runtime/actions.js');
  setActionPage('pages/home/index');
  configureActions(catalog);
  const {actions}=createView({},{});
  assert.throws(()=>actions.holdPage({}),{message:'UI_ACTION: `holdPage` is not an action of pages/home/index: the build did not see it'});
  assert.equal(actions.then,undefined);
  assert.equal(actions[Symbol.iterator],undefined);
  assert.equal(await Promise.resolve(actions),actions);
  assert.equal(JSON.stringify(actions),'{}');
  assert.equal('holdPage' in actions,false);
  assert.deepEqual(Object.keys(actions),['increment']);
});

test('a read before the catalog arrives waits as undefined; the same read after it names the missing action', async()=>{
  const {setActionPage,resetActions,currentActions}=await import('../src/runtime/actions.js');
  resetActions();setActionPage('pages/home/index');
  assert.equal(currentActions().actions.holdPage,undefined);
  configureActions(catalog);
  assert.throws(()=>currentActions().actions.holdPage,{message:'UI_ACTION: `holdPage` is not an action of pages/home/index: the build did not see it'});
});
