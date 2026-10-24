import {test} from 'node:test';
import assert from 'node:assert/strict';
import {h, prepare, prepareWindows, commit, discard, dispatch, useLocal, reset} from '../src/runtime.js';
import {listFields} from '../src/schema.js';
function records(buffer) {
 const result=[];
 for(let i=0;i<buffer.length;) { const fields=new Map(); for(let n=0;n<buffer[i+5];n++)fields.set(buffer[i+6+n*2],buffer[i+7+n*2]);result.push({tag:buffer[i],id:buffer[i+3],text:buffer[i+4],fields});i+=6+buffer[i+5]*2; }
 return result;
}
function window(start,end) { return {root:{start,end,leading:start*100,trailing:(1000-end)*100}}; }
function row(i) { const value=useLocal('count',0);return h('button',{testId:`row-${i}`,onClick:()=>value.set(value.value+1)},`${i}:${value.value}`); }

test('keyed factory hooks require identity before execution and retain it across reorder',()=>{
 reset();let items=['a','b'],keyed=false;
 const page=()=>h('list',{count:items.length,itemKeys:keyed?items:undefined,row:i=>{
  const n=useLocal('n',0);return h('button',{key:items[i],testId:items[i],onClick:()=>n.set(n.value+1)},`${items[i]}:${n.value}`);
 }});
 assert.throws(()=>prepare(page,{}, {root:{start:0,end:items.length,leading:0,trailing:0}}),/UI_LIST: keyed row factory hooks require itemKeys/);
 keyed=true;
 let output=records(prepare(page,{}, {root:{start:0,end:items.length,leading:0,trailing:0}}));commit();
 dispatch(output.find(r=>r.id==='a').fields.get(8));
 prepare(page,{}, {root:{start:0,end:items.length,leading:0,trailing:0}});commit();
 items=['b','a'];output=records(prepare(page,{}, {root:{start:0,end:items.length,leading:0,trailing:0}}));commit();
 assert.deepEqual(output.filter(r=>r.tag===3).map(r=>r.text),['b:0','a:1']);
 items=['b'];prepare(page,{}, {root:{start:0,end:items.length,leading:0,trailing:0}});discard();
 items=['b','a'];output=records(prepare(page,{}, {root:{start:0,end:items.length,leading:0,trailing:0}}));commit();
 assert.equal(output.find(r=>r.id==='a').text,'a:1');
 items=['b'];prepare(page,{}, {root:{start:0,end:items.length,leading:0,trailing:0}});commit();
 items=['b','a'];output=records(prepare(page,{}, {root:{start:0,end:items.length,leading:0,trailing:0}}));commit();
 assert.equal(output.find(r=>r.id==='a').text,'a:0');
});

test('cross-window anchors reuse old geometry until native refines the destination',()=>{
 reset();let anchor,visited=[];
 const page=()=>h('list',{count:1000,anchor,row:i=>{visited.push(i);return row(i);}});
 let output=records(prepare(page,{},window(0,3)));commit();
 const original=output.find(r=>r.id==='row-1').fields.get(8);
 anchor={index:999,align:'end',seq:1};visited=[];
 output=records(prepare(page,{},window(0,3)));
 assert.deepEqual(visited,[]);
 assert.deepEqual(output.filter(r=>r.id).map(r=>r.id),['row-0','row-1','row-2']);
 discard();assert.equal(dispatch(original),true);
 prepare(page,{},window(0,3));
 output=records(prepareWindows(window(997,1000)));commit();
 assert.deepEqual(visited,[997,998,999]);
 assert.equal(output.find(r=>r.id==='row-999').text,'999:0');
 assert.equal(dispatch(original),false);
 visited=[];
 prepare(page,{},window(997,1000));commit();
 assert.deepEqual(visited,[997,998,999],'unchanged anchor keeps ordinary rendering');
 anchor={index:998,align:'end',seq:2};visited=[];
 prepare(page,{},window(997,1000));commit();
 assert.deepEqual(visited,[997,998,999],'an in-window anchor needs no deferred rows');
 anchor={index:0,align:'start',seq:3};visited=[];
 prepare(page,{},window(997,1000));
 output=records(prepareWindows(window(0,3)));commit();
 assert.deepEqual(visited,[0,1,2]);
 assert.equal(output.find(r=>r.id==='row-1').text,'1:1','offscreen row locals survive the jump');
});

test('anchor refinement reevaluates overlapping rows with the new render props',()=>{
 reset();let anchor,version='old',visited=[];
 const page=()=>h('list',{count:1000,anchor,row:i=>{visited.push(i);return h('text',{testId:`row-${i}`},`${version}:${i}`);}});
 prepare(page,{},window(0,3));commit();
 version='new';anchor={index:3,align:'start'};visited=[];
 prepare(page,{},window(0,3));
 assert.deepEqual(visited,[]);
 const output=records(prepareWindows(window(1,5)));commit();
 assert.deepEqual(visited,[1,2,3,4]);
 assert.deepEqual(output.filter(r=>r.id).map(r=>r.text),['new:1','new:2','new:3','new:4']);
});

test('window shifts only construct entering rows and retain row local state',()=>{
 reset();let roots=0,rows=0;
 function Item({i}) {return row(i);}
 const page=()=>{roots++;return h('list',{count:1000,height:()=>100,row:i=>{rows++;return h(Item,{key:String(i),i});}});};
 let output=records(prepare(page,{},window(0,3)));commit();
 assert.equal(roots,1);assert.equal(rows,3);assert.equal(output.filter(x=>x.tag===3).length,3);
 dispatch(output.find(x=>x.id==='row-1').fields.get(8));
 output=records(prepare(page,{},window(0,3)));commit();
 assert.equal(output.find(x=>x.id==='row-1').text,'1:1');
 const before=rows;
 output=records(prepareWindows(window(2,5)));commit();
 assert.equal(roots,2);assert.equal(rows,before+2);
 assert.equal(output.find(x=>x.id==='row-1'),undefined);
 output=records(prepareWindows(window(0,3)));commit();
 assert.equal(output.find(x=>x.id==='row-1').text,'1:1');
 assert.equal(roots,2);
});

test('failed row candidates preserve the old window and callbacks',()=>{
 reset();let fail=false;
 const page=()=>h('list',{count:1000,row:i=>{if(fail&&i===4)throw Error('row failed');return h('button',{onClick(){}},String(i));}});
 const first=records(prepare(page,{},window(0,3)));commit();
 const old=first.find(x=>x.tag===3).fields.get(8);
 fail=true;assert.throws(()=>prepareWindows(window(2,5)),/row failed/);
 assert.equal(dispatch(old),true);
 fail=false;prepareWindows(window(2,5));discard();assert.equal(dispatch(old),true);
 prepareWindows(window(2,5));commit();assert.equal(dispatch(old),false);
});

test('first-window refinement does not rerender the page and shrinking releases removed row state',()=>{
 reset();let roots=0,count=1000;
 function Item({i}){return row(i);}
 const page=()=>{roots++;return h('list',{count,row:i=>h(Item,{key:String(i),i})});};
 prepare(page,{});let output=records(prepareWindows(window(7,9)));commit();assert.equal(roots,1);
 dispatch(output.find(x=>x.id==='row-8').fields.get(8));prepare(page,{},window(7,9));commit();
 count=2;prepare(page,{}, {root:{start:0,end:2,leading:0,trailing:0}});commit();
 count=1000;output=records(prepare(page,{},window(7,9)));commit();
 assert.equal(output.find(x=>x.id==='row-8').text,'8:0');
 assert.equal(output[0].fields.get(listFields.listCount),1000);
});

test('invalid descriptions and windows cannot replace a committed list',()=>{
 reset();
 const page=()=>h('list',{count:1000,row:i=>h('button',{onClick(){}},String(i))});
 const first=records(prepare(page,{},window(0,2)));commit();
 const handle=first.find(x=>x.tag===3).fields.get(8);
 for(const props of [
  {count:-1,row:()=>h('text')}, {count:100001,row:()=>h('text')},
  {count:1,row:42}, {count:1,row:()=>h('text'),height:()=>NaN},
  {count:1,row:()=>h('text'),anchor:{index:0,align:'center'}},
 ]) {
  assert.throws(()=>prepare(()=>h('list',props),{}),/UI_LIST/);
  assert.equal(dispatch(handle),true);
 }
 assert.throws(()=>prepareWindows({root:{start:2,end:1,leading:0,trailing:0}}),/UI_LIST/);
 assert.equal(dispatch(handle),true);
 assert.throws(()=>prepare(()=>h('list',{count:2,row:()=>h('text',{key:'duplicate'},'x')}),{},window(0,2)),/UI_KEY/);
 assert.equal(dispatch(handle),true);
});

test('reach callbacks are retained handles and never run during candidate preparation',()=>{
 reset();let starts=0,ends=0;
 const page=()=>h('list',{count:1000,onReachStart:()=>starts++,onReachEnd:()=>ends++,row:i=>h('text',{},String(i))});
 let output=records(prepare(page,{},window(0,2)));commit();
 const start=output[0].fields.get(17),end=output[0].fields.get(18);
 assert.deepEqual([starts,ends],[0,0]);
 output=records(prepareWindows(window(5,7)));commit();
 assert.equal(output[0].fields.get(17),start);
 assert.equal(dispatch(start),true);assert.equal(dispatch(end),true);
 assert.deepEqual([starts,ends],[1,1]);
 prepare(page,{},window(5,7));discard();assert.equal(dispatch(start),true);
 prepare(page,{},window(5,7));commit();assert.equal(dispatch(start),false);
 assert.throws(()=>prepare(()=>h('scroll',{onReachEnd:42}),{}),/UI_PROP/);
 assert.throws(()=>prepare(()=>h('column',{onReachStart(){}}),{}),/UI_PROP/);
});


test('shrinking a list accepts the previous native window before refinement',()=>{
 reset();let count=1000;
 const page=()=>h('list',{count,row:i=>h('text',{},String(i))});
 prepare(page,{},window(997,1000));commit();
 count=1;
 let output=records(prepare(page,{},window(997,1000)));commit();
 assert.equal(output[0].fields.get(listFields.listCount),1);
 assert.deepEqual(output.filter(x=>x.tag===2).map(x=>x.text),['0']);
 count=0;output=records(prepare(page,{},window(0,1)));commit();
 assert.equal(output.length,1);assert.equal(output[0].fields.get(listFields.leading),0);assert.equal(output[0].fields.get(listFields.trailing),0);
 count=1000;prepare(page,{}, {root:{start:0,end:0,leading:0,trailing:0}});
 output=records(prepareWindows(window(0,3)));commit();assert.equal(output.filter(x=>x.tag===2).length,3);
 assert.throws(()=>prepareWindows(window(0,1001)),/UI_LIST/);
});


test('nested list refinement isolates discarded windows and retains ordinary callbacks',()=>{
 reset();let calls=0,fail=false;
 const page=()=>h('column',{},
  h('button',{testId:'ordinary',onClick:()=>calls++},'Ordinary'),
  h('list',{count:1,row:()=>h('column',{},h('list',{count:10,row:i=>{
   if(fail&&i===4)throw Error('nested row failed');
   return h('button',{testId:`nested-${i}`,onClick:()=>calls++},String(i));
  }}))}));
 const first=records(prepare(page,{}));commit();
 const nestedId=first.filter(r=>r.tag===6).at(-1).fields.get(listFields.listId);
 const ordinary=first.find(r=>r.id==='ordinary').fields.get(8);
 const original=first.find(r=>r.id==='nested-0').fields.get(8);
 const shift={ [nestedId]:{start:3,end:5,leading:300,trailing:500} };
 fail=true;assert.throws(()=>prepareWindows(shift),/nested row failed/);
 fail=false;
 let output=records(prepareWindows({}));
 assert.deepEqual(output.filter(r=>r.id.startsWith('nested-')).map(r=>r.id),['nested-0']);
 discard();assert.equal(dispatch(original),true);
 prepareWindows(shift);discard();
 output=records(prepareWindows({}));commit();
 assert.equal(output.find(r=>r.id==='nested-0').fields.get(8),original);
 output=records(prepareWindows(shift));commit();
 assert.deepEqual(output.filter(r=>r.id.startsWith('nested-')).map(r=>r.id),['nested-3','nested-4']);
 assert.equal(dispatch(original),false);
 assert.equal(output.find(r=>r.id==='ordinary').fields.get(8),ordinary);
 assert.equal(dispatch(ordinary),true);assert.equal(calls,2);
});

test('complete item keys describe offscreen lifetime without constructing offscreen rows',()=>{
 reset();let keys=['alpha','beta','gamma','delta'],visited=[];
 const page=()=>h('list',{count:keys.length,itemKeys:keys,row:i=>{
  visited.push(i);const value=useLocal('count',0);
  return h('button',{testId:keys[i],onClick:()=>value.set(value.value+1)},`${keys[i]}:${value.value}`);
 }});
 let output=records(prepare(page,{},window(1,2)));commit();
 assert.deepEqual(visited,[1]);
 assert.deepEqual(JSON.parse(output[0].fields.get(listFields.itemKeys)),keys);
 dispatch(output.find(r=>r.id==='beta').fields.get(8));
 keys=['beta','alpha','delta'];visited=[];
 output=records(prepare(page,{},window(0,1)));commit();
 assert.deepEqual(visited,[0]);
 assert.equal(output.find(r=>r.id==='beta').text,'beta:1','local identity follows key across row reorder');
 assert.deepEqual(JSON.parse(output[0].fields.get(listFields.itemKeys)),['beta','alpha','delta'],'offscreen removal reaches native');
 keys[2]='mutated';
 output=records(prepareWindows(window(0,1)));discard();
 assert.deepEqual(JSON.parse(output[0].fields.get(listFields.itemKeys)),['beta','alpha','delta'],'candidate snapshots the roster');
});

test('item key validation and failed anchor candidates preserve committed rows',()=>{
 reset();let keys=['alpha','beta'],anchor;
 const page=()=>h('list',{count:keys.length,itemKeys:keys,anchor,row:i=>h('button',{key:keys[i],testId:keys[i],onClick(){}},keys[i])});
 let output=records(prepare(page,{},window(0,1)));commit();
 const original=output.find(r=>r.id==='alpha').fields.get(8);
 keys=['replacement','beta'];anchor={index:1,align:'end',seq:1};
 output=records(prepare(page,{},window(0,1)));
 assert.equal(output.find(r=>r.id==='replacement').text,'replacement','anchor seed cannot emit an old row under a new key');
 discard();assert.equal(dispatch(original),true);
 for(const itemKeys of [['same','same'],['only-one'],['ok',null],new Array(2),['ok','图'.repeat(100)],['ok','\ud800']]) {
  assert.throws(()=>prepare(()=>h('list',{count:2,itemKeys,row:i=>h('text',{},String(i))}),{}),/UI_LIST/);
 }
 assert.throws(()=>prepare(()=>h('list',{count:1,itemKeys:['declared'],row:()=>h('text',{key:'different'},'x')}),{}),/row key does not match/);
 assert.equal(dispatch(original),true);
});

test('item keys enforce a page-wide UTF-8 budget before lazy row construction',()=>{
 reset();const itemKeys=Array.from({length:1200},(_,i)=>String(i).padEnd(200,'x'));
 const list=()=>h('list',{count:itemKeys.length,itemKeys,row:i=>h('text',{},String(i))});
 assert.doesNotThrow(()=>prepare(list,{}));discard();
 assert.throws(()=>prepare(()=>h('column',{},list(),list()),{}),/itemKeys page budget/);
 const oversized=[...itemKeys,...itemKeys.map(key=>'y'+key)];
 assert.throws(()=>prepare(()=>h('list',{count:oversized.length,itemKeys:oversized,row:i=>h('text',{},String(i))}),{}),/itemKeys budget/);
});

test('offscreen keyed row state survives reorder but is released on semantic removal',()=>{
 reset();let keys=['alpha','beta','gamma'];
 const page=()=>h('list',{count:keys.length,itemKeys:keys,row:i=>{
  const value=useLocal('count',0);
  return h('button',{testId:keys[i],onClick:()=>value.set(value.value+1)},`${keys[i]}:${value.value}`);
 }});
 let output=records(prepare(page,{},window(1,2)));commit();
 dispatch(output.find(r=>r.id==='beta').fields.get(8));
 prepare(page,{},window(0,1));commit();
 keys=['alpha','gamma','beta'];prepare(page,{},window(0,1));commit();
 output=records(prepareWindows(window(2,3)));commit();
 assert.equal(output.find(r=>r.id==='beta').text,'beta:1');
 prepareWindows(window(0,1));commit();
 keys=['alpha','gamma'];prepare(page,{},window(0,1));commit();
 keys=['alpha','gamma','beta'];output=records(prepare(page,{},window(2,3)));commit();
 assert.equal(output.find(r=>r.id==='beta').text,'beta:0','removed identity cannot resurrect its old local state');
});
