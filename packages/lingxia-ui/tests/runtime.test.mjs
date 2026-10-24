import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, describe, prepare, commit, discard, dispatch, useLocal, createView } from '../src/runtime.js';

test('image lifecycle callbacks are committed handles and retry keys are bounded', () => {
  let loads=0, errors=0;
  const render=()=>h('image',{src:'photo.png',retryKey:'attempt-1',onLoad:()=>loads++,onError:()=>errors++});
  const buffer=describe(render,{}), fields=new Map();
  for(let i=6;i<buffer.length;i+=2) fields.set(buffer[i],buffer[i+1]);
  assert.equal(fields.get(106),'attempt-1');
  prepare(render,{}); discard();
  assert.equal(dispatch(fields.get(107)),true); assert.equal(loads,1);
  assert.equal(dispatch(fields.get(108)),true); assert.equal(errors,1);
  describe(render,{}); assert.equal(dispatch(fields.get(107)),false);
  for(const retryKey of [3,'x'.repeat(257),'\0'])
    assert.throws(()=>describe(()=>h('image',{src:'photo.png',retryKey}),{}),/UI_IMAGE/);
  assert.throws(()=>describe(()=>h('image',{src:'photo.png',onError:true}),{}),/UI_IMAGE/);
});

test('image alternative text distinguishes descriptions from decorative images and bounds UTF-8', () => {
  for (const alt of ['', 'A mountain lake', '图片'.repeat(170)]) {
    const buffer = describe(() => h('image', {src:'photo.png', alt}), {});
    assert.deepEqual(buffer.slice(-2), [105, alt]);
  }
  for (const alt of [null, 1, true, {}, '   ', '\0', '图片'.repeat(171), '\ud800'])
    assert.throws(() => describe(() => h('image', {src:'photo.png', alt}), {}), /UI_IMAGE/);
  assert.doesNotThrow(() => describe(() => h('image', {src:'legacy.png'}), {}));
  assert.throws(() => describe(() => h('text', {alt:'Wrong primitive'}, 'Text'), {}), /UI_PROP/);
});

test('flex basis encodes auto, logical lengths and percentages without mixing them', () => {
  for(const value of ['auto','0%','50%',0,48]) {
    const buffer=describe(()=>h('column',{style:{flexBasis:value}}),{});
    assert.deepEqual(buffer.slice(6),[99,value]);
  }
  for(const value of [-1,Infinity,NaN,16385,'-1%','1e2%','auto ','10px','content',null])
    assert.throws(()=>describe(()=>h('column',{style:{flexBasis:value}}),{}),/UI_STYLE_VALUE/);
});

test('vectors encode bounded path data and can decorate composed buttons', () => {
  const paths=[{d:'M2 2h20v20H2z',fill:'none',stroke:'currentColor',strokeWidth:1.5}];
  const buffer=describe(()=>h('vector',{paths}),{});
  assert.equal(buffer[0],19);
  assert.equal(buffer[6],80);
  assert.deepEqual(JSON.parse(buffer[7]),{viewBox:[0,0,24,24],paths});
  assert.doesNotThrow(()=>describe(()=>h('button',{accessibilityLabel:'Open'},h('vector',{paths})),{}));
  for (const props of [{paths:[]},{paths:[{d:''}]},{paths:[{d:'M0 0',href:'remote'}]},{paths,viewBox:[0,0,0,24]},{paths:[{d:'M0 0',strokeWidth:Infinity}]},{paths:[{d:'M0 0',fill:'url(remote)'}]}])
    assert.throws(()=>describe(()=>h('vector',props),{}),/UI_VECTOR/);
  assert.throws(()=>describe(()=>h('vector',{paths},h('text',{},'child')),{}),/UI_CHILD/);
  assert.throws(()=>describe(()=>h('vector',{paths,onClick(){}}),{}),/UI_PROP/);
});

test('percentage dimensions encode separate append-only fields and reject malformed lengths', () => {
  for (const [index,name] of ['width','height','minWidth','minHeight','maxWidth','maxHeight'].entries()) {
    const buffer = describe(() => h('column',{style:{[name]:'72%'}}),{});
    assert.deepEqual(buffer.slice(6),[74+index,72]);
    for (const value of [-1,NaN,Infinity,null,...(name === 'height' ? [] : ['auto']),'-1%','2px','1e2%','16385%','50 %','10%junk'])
      assert.throws(() => describe(() => h('column',{style:{[name]:value}}),{}),/UI_STYLE_VALUE/);
  }
  assert.deepEqual(describe(() => h('column',{style:{width:100}}),{}).slice(6),[0,100]);
  assert.throws(() => describe(() => h('column',{style:{widthPercent:50}}),{}),/UI_STYLE/);
});

test('outer shadow wire values are bounded and reject unsupported members', () => {
  const buffer = describe(() => h('column',{style:{boxShadow:[{offsetY:2,blur:4}]}}),{});
  assert.equal(buffer[6],73);
  assert.deepEqual(JSON.parse(buffer[7]),[{offsetX:0,offsetY:2,blur:4,spread:0,color:'currentColor'}]);
  for (const value of [null,{},'red',[{inset:true}],[{blur:-1}],[{blur:257}],[{spread:1025}],[{offsetX:null}],[{color:'red'}],Array(9).fill({})])
    assert.throws(() => describe(() => h('column',{style:{boxShadow:value}}),{}),/UI_SHADOW/);
  assert.equal(describe(() => h('column',{style:{boxShadow:'none'}}),{})[7],'none');
});

test('linear gradients encode bounded immutable values and reject invalid styles', () => {
  const gradient = {direction:'right',stops:[{offset:0,color:'#ff0000'},{offset:1,color:'#0000ff80'}]};
  const buffer = describe(() => h('column', {style:{backgroundGradient:gradient}}), {});
  assert.equal(buffer[6], 72);
  assert.deepEqual(JSON.parse(buffer[7]), {...gradient,colorSpace:'srgb'});
  for (const value of [null, 'red', {}, {...gradient, angle:30}, {...gradient, unknown:true}, {...gradient,colorSpace:'hsl'}, {...gradient,stops:[]}, {...gradient,stops:[{offset:1,color:'#ff0000'},{offset:0,color:'#0000ff'}]}, {...gradient,angle:Infinity,direction:undefined}])
    assert.throws(() => describe(() => h('column', {style:{backgroundGradient:value}}), {}), /UI_GRADIENT/);
  assert.equal(describe(() => h('column', {style:{backgroundGradient:'none'}}), {})[7], 'none');
});

test('property admission stays local to each primitive and rejects prototype names', () => {
  for (const [tag, property] of [
    ['text', 'onClick'], ['button', 'onInput'], ['column', 'enabled'],
    ['row', 'row'], ['scroll', 'count'], ['list', 'onClick'],
    ['image', 'columns'], ['grid', 'src'], ['plot', 'onChange'],
    ['input', 'onClick'], ['textarea', 'onChange'], ['search', 'options'],
    ['checkbox', 'min'], ['switch', 'placeholder'], ['slider', 'secure'],
    ['radio', 'step'], ['spacer', 'lines'], ['stack', 'onReachStart'],
  ]) {
    assert.throws(() => describe(() => h(tag, { [property]: 1 }), {}), /UI_PROP:/);
    for (const key of ['constructor', 'toString', '__proto__']) {
      assert.throws(() => describe(() => h(tag, { [key]: 1 }), {}), /UI_PROP:/);
    }
  }
});

test('flattens conditional children and lowers text into flat primitive records', () => {
  const buffer = describe(() => h('column', {}, false, null, [h('text', { testId: 'hello' }, 'Hello ', 42)]), {});
  assert.deepEqual(buffer, [0, -1, '', '', '', 0, 2, 0, '', 'hello', 'Hello 42', 0]);
});

test('padding edge values reject invalid lengths before native layout', () => {
  for (const name of ['padding', 'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft']) {
    for (const value of [-1, 16385, Infinity, NaN, '12', null, true])
      assert.throws(() => describe(() => h('column', { style: { [name]: value } }), {}), /UI_STYLE_VALUE/);
    assert.doesNotThrow(() => describe(() => h('column', { style: { [name]: 0 } }), {}));
  }
});
test('components execute during description with the supplied View', () => {
  const view = { data: { greeting: 'Hello' } };
  let called = false;
  function Greeting(props, actual) { called = true; assert.equal(actual, view); return h('text', {}, props.prefix, actual.data.greeting); }
  const result = describe(() => { const node = h(Greeting, { prefix: '> ' }); assert.equal(called, false); return node; }, view);
  assert.equal(result[4], '> Hello');
});
test('rejects unsupported tags, CSS, callbacks and invalid child shapes', () => {
  for (const render of [() => h('div'), () => h('toString'), () => h('text', {style: {className:'x'}}), () => h('button', {onClick:42}), () => h('text', {}, {}), () => h('spacer', {}, 'x'), () => [h('text')]]) {
    assert.throws(() => describe(render, {}), /UI_/);
  }
});
test('scroll retains child button callbacks and rejects unsupported scroll props', () => {
  const buffer = describe(() => h('scroll', { testId: 'feed', style: { height: 100 } },
    h('button', { key: 'item', onClick() {} }, 'Item')), {});
  assert.equal(buffer[0], 5);
  assert.equal(buffer[3], 'feed');
  assert.equal(buffer[8], 3);
  assert.equal(buffer[9], 0);
  assert.throws(() => describe(() => h('scroll', { onClick() {} }), {}), /UI_PROP/);
});

function buttons(buffer) {
  const result = [];
  for (let i = 0; i < buffer.length;) {
    const fields = new Map();
    for (let n = 0; n < buffer[i + 5]; n++) fields.set(buffer[i + 6 + n * 2], buffer[i + 7 + n * 2]);
    if (buffer[i] === 3) result.push({ text: buffer[i + 4], handle: fields.get(8) });
    i += 6 + buffer[i + 5] * 2;
  }
  return result;
}
test('keyed components retain local state across reorder and release it on removal', () => {
  function Counter({name}) {
    const count = useLocal('count', 0);
    return h('button', {onClick: () => count.set(count.value + 1)}, `${name}:${count.value}`);
  }
  let names = ['a', 'b'];
  const render = () => h('column', {}, names.map(name => h(Counter, {key:name, name})));
  let nodes = buttons(describe(render, {}));
  assert.equal(dispatch(nodes[0].handle), true);
  names = ['b', 'a'];
  let next = buttons(describe(render, {}));
  assert.deepEqual(next.map(n => n.text), ['b:0', 'a:1']);
  assert.equal(dispatch(nodes[0].handle), false);
  names = ['b']; describe(render, {});
  names = ['a', 'b']; next = buttons(describe(render, {}));
  assert.equal(next[0].text, 'a:0');
});
test('discarded candidates preserve committed callbacks and local state', () => {
  const render = () => {
    const count = useLocal('count', 0);
    return h('button', {onClick: () => count.set(count.value + 1)}, count.value);
  };
  const first = buttons(describe(render, {}))[0];
  dispatch(first.handle);
  assert.equal(buttons(prepare(render, {}))[0].text, '1');
  discard();
  assert.equal(dispatch(first.handle), true);
  assert.equal(buttons(prepare(render, {}))[0].text, '1');
  commit();
  assert.equal(dispatch(first.handle), false);
});
test('disabled controls and render-time mutations cannot dispatch', () => {
  assert.equal(buttons(describe(() => h('button', {enabled:false,onClick(){throw Error('called');}}, 'Disabled'), {}))[0].handle, undefined);
  assert.throws(() => describe(() => { useLocal('x', 0).set(1); return h('text'); }, {}), /UI_RENDER/);
});

test('replicated data and environment are deeply immutable without prototype mutation', () => {
  const data = JSON.parse('{"nested":{"value":1},"rows":[{"text":"first"}],"__proto__":{"injected":true}}');
  const view = createView(data,{viewport:{width:300,height:500}});
  assert.throws(() => { view.data.nested.value=2; },TypeError);
  assert.throws(() => { view.data.rows[0].text='changed'; },TypeError);
  assert.throws(() => { view.data.rows.push({text:'new'}); },TypeError);
  assert.throws(() => { view.env.viewport.width=1; },TypeError);
  assert.equal(Object.getPrototypeOf(view.data),Object.prototype);
  assert.equal(view.data.injected,undefined);
  assert.equal(Object.hasOwn(view.data,'__proto__'),true);
});
test('a retained candidate cannot alias a new render arena', () => {
  let old;
  describe(() => old = h('text', {}, 'old'), {});
  assert.throws(() => describe(() => { h('text', {}, 'new'); return old; }, {}), /UI_STALE/);
  assert.equal(describe(() => h('text', {}, 'recovered'), {})[4], 'recovered');
});


test('image props lower to primitive fields and reject invalid sources, fits and children', () => {
  const buffer = describe(() => h('image', {src:'assets/avatar.png', fit:'contain', style:{width:64,height:64}}), {});
  assert.equal(buffer[0], 7);
  assert.ok(buffer.includes('assets/avatar.png'));
  assert.ok(buffer.includes('contain'));
  for (const props of [{}, {src:42}, {src:''}, {src:'ok',fit:'stretch'}, {src:'a\0b'}])
    assert.throws(() => describe(() => h('image', props), {}), /UI_IMAGE/);
  assert.throws(() => describe(() => h('image', {src:'a'}, h('text',{},'x')), {}), /UI_CHILD/);
  assert.throws(() => describe(() => h('column', {src:'a'}), {}), /UI_PROP/);
});

test('layout alignment enums are checked before committing a candidate', () => {
  const buffer=describe(() => h('row', {style:{minWidth:80,maxWidth:320,minHeight:40,maxHeight:100,shrink:1,align:'center',justify:'between',alignSelf:'end'}}), {});
  assert.equal(buffer[5],8);
  for (const style of [{align:'between'},{justify:'stretch'},{alignSelf:'auto'},{align:1}]) {
    assert.throws(() => describe(() => h('column',{style}),{}), /UI_STYLE_VALUE/);
  }
  for (const style of [{align:'center'},{justify:'end'}]) {
    assert.throws(() => describe(() => h('list',{count:0,row:()=>h('text',{},'row'),style}),{}), /UI_LIST/);
  }
});

test('typography validates numeric weight, absolute line height and generic family', () => {
  assert.doesNotThrow(() => describe(() => h('text',{style:{fontWeight:650,lineHeight:24,fontFamily:'mono'}},'Hello'),{}));
  for (const style of [{fontWeight:0},{fontWeight:1001},{fontWeight:'bold'},{fontWeight:NaN},{lineHeight:0},{lineHeight:Infinity},{lineHeight:'24'},{fontFamily:'Arial'}]) {
    assert.throws(() => describe(() => h('text',{style},'Hello'),{}), /UI_STYLE/);
  }
});

function checkRecord(buffer) {
  return { tag:buffer[0], text:buffer[4], fields:new Map(Array.from({length:buffer[5]}, (_,i)=>[buffer[6+i*2],buffer[7+i*2]])) };
}

test('boolean controls dispatch the inverse controlled value and expire old handles', () => {
  for (const tag of ['checkbox','switch']) {
    let received;
    const render = () => h(tag, {value:true,onChange:value=>{received=value;}}, 'Preference');
    const first=checkRecord(describe(render,{}));
    assert.equal(first.text,'Preference'); assert.equal(first.fields.get(39),1);
    assert.equal(dispatch(first.fields.get(40)),true); assert.equal(received,false);
    const next=checkRecord(describe(render,{}));
    assert.equal(next.fields.get(39),1);
    assert.equal(dispatch(first.fields.get(40)),false);
    assert.equal(checkRecord(describe(()=>h(tag,{value:true,enabled:false,onChange(){throw Error('disabled');}}),{})).fields.has(40),false);
  }
});

test('boolean control local bindings persist and reject invalid values or children', () => {
  const render=()=>h('switch',{bind:useLocal('preference',false)},'Preference');
  const first=checkRecord(describe(render,{}));
  assert.equal(first.fields.get(39),0); assert.equal(dispatch(first.fields.get(40)),true);
  assert.equal(checkRecord(describe(render,{})).fields.get(39),1);
  for (const tag of ['checkbox','switch']) {
    for (const props of [{value:1},{value:'true'},{onChange:2},{bind:{value:false,set(){}}}]) {
      assert.throws(()=>describe(()=>h(tag,props),{}),/UI_/);
    }
    for (const value of [undefined,null,0,'false']) assert.throws(()=>describe(()=>h(tag,{bind:useLocal('invalid',value)}),{}),/UI_/);
    assert.throws(()=>describe(()=>h(tag,{bind:useLocal('valid',false),value:false}),{}),/UI_/);
    assert.throws(()=>describe(()=>h(tag,{},h('text',{},'child')),{}),/UI_/);
  }
});

function radioRecords(buffer) {
  const result=[];
  for(let i=0;i<buffer.length;){const count=buffer[i+5];result.push({tag:buffer[i],key:buffer[i+2],text:buffer[i+4],fields:new Map(Array.from({length:count},(_,n)=>[buffer[i+6+n*2],buffer[i+7+n*2]]))});i+=6+count*2;}
  return result;
}
test('radio shares a string binding, retains selection across reorder, and ignores selecting the selected option',()=>{
  let options=[{value:'daily',label:'Daily'},{value:'weekly',label:'Weekly'},{value:'never',label:'Never',enabled:false}];
  const render=()=>h('radio',{bind:useLocal('pace','daily'),options,accessibilityLabel:'Pace'});
  let rows=radioRecords(describe(render,{}));
  assert.equal(rows[0].tag,12);assert.equal(rows[0].fields.get(41),'Pace');
  assert.deepEqual(rows.slice(1).map(r=>r.fields.get(39)),[1,0,0]);
  const stale=rows[2].fields.get(40);assert.equal(dispatch(stale),true);
  options=[options[1],options[0],options[2]];
  rows=radioRecords(describe(render,{}));assert.deepEqual(rows.slice(1).map(r=>r.fields.get(39)),[1,0,0]);
  assert.equal(rows[1].key,'weekly');assert.equal(rows[3].fields.has(40),false);assert.equal(dispatch(stale),false);
  assert.equal(dispatch(rows[1].fields.get(40)),true);
  rows=radioRecords(describe(render,{}));assert.equal(rows[1].fields.get(39),1);
});
test('radio is controlled, validates options and binding, and disables the entire group',()=>{
  const options=[{value:'a',label:'A'},{value:'b',label:'B'}];let changed=[];
  const render=()=>h('radio',{value:'a',options,onChange:value=>changed.push(value)});
  let rows=radioRecords(describe(render,{}));dispatch(rows[1].fields.get(40));assert.deepEqual(changed,[]);
  dispatch(rows[2].fields.get(40));assert.deepEqual(changed,['b']);assert.equal(radioRecords(describe(render,{}))[1].fields.get(39),1);
  rows=radioRecords(describe(()=>h('radio',{options,enabled:false,onChange(){throw Error('disabled')}}),{}));assert.ok(rows.slice(1).every(r=>!r.fields.has(40)&&r.fields.get(9)===0));
  for(const props of [{options:[]},{options:[options[0],options[0]]},{options:Array.from({length:65},(_,i)=>({value:String(i),label:'x'}))},{options,value:1},{options,onChange:1},{options:[{value:'a',label:1}]},{options,bind:{value:'a',set(){}}}]) assert.throws(()=>describe(()=>h('radio',props),{}),/UI_/);
  assert.throws(()=>describe(()=>h('radio',{options,bind:useLocal('bad',false)}),{}),/UI_/);
  assert.throws(()=>describe(()=>h('radio',{options,bind:useLocal('good','a'),value:'a'}),{}),/UI_/);
  assert.throws(()=>describe(()=>h('radio-option',{},'forged'),{}),/UI_TAG/);
  assert.throws(()=>describe(()=>h('radio',{options},h('text',{},'child')),{}),/UI_CHILD/);
});


test('grid validates bounded columns and preserves keyed child order during reflow',()=>{
  for(const columns of [undefined,null,0,-1,1.5,65,Infinity,'2']) assert.throws(()=>describe(()=>h('grid',{columns}),{}),/UI_GRID/);
  assert.throws(()=>describe(()=>h('column',{columns:2}),{}),/UI_PROP/);
  for(const columns of [1,3,64]) {
    const rows=radioRecords(describe(()=>h('grid',{columns},h('text',{key:'a'},'A'),h('text',{key:'b'},'B')),{}));
    assert.equal(rows[0].tag,16);assert.equal(rows[0].fields.get(47),columns);
    assert.deepEqual(rows.slice(1).map(row=>row.key),['a','b']);
  }
});


test('stack offsets lower through components and reject non-stack parents or malformed values',()=>{
 const Child=()=>h('button',{style:{offset:{x:-5,y:12}}},'Top');
 const rows=radioRecords(describe(()=>h('stack',{},h('text',{},'Back'),h(Child,{})),{}));
 assert.equal(rows[0].tag,17);assert.equal(rows[2].fields.get(48),-5);assert.equal(rows[2].fields.get(49),12);
 for(const offset of [null,4,[],{x:null},{x:Infinity},{y:-16385},{z:1}])
  assert.throws(()=>describe(()=>h('stack',{},h('text',{style:{offset}},'x')),{}),/UI_STACK/);
 for(const parent of ['column','row','grid'])
  assert.throws(()=>describe(()=>h(parent,{...(parent==='grid'?{columns:1}:{})},h(Child,{})),{}),/UI_STACK/);
 assert.throws(()=>describe(()=>h('stack',{style:{offset:{x:1}}}),{}),/UI_STACK/);
 assert.throws(()=>describe(()=>h('stack',{},h('text',{style:{offsetX:1}},'x')),{}),/UI_STYLE/);
});


test('text line caps are bounded leaf metadata and preserve the source string',()=>{
 const rows=radioRecords(describe(()=>h('text',{lines:2},'First\nSecond\nThird'),{}));
 assert.equal(rows[0].fields.get(50),2);assert.equal(rows[0].text,'First\nSecond\nThird');
 for(const lines of [null,0,-1,1.5,1025,Infinity,'2',true])assert.throws(()=>describe(()=>h('text',{lines},'x'),{}),/UI_TEXT/);
 for(const tag of ['button','input','column','stack'])assert.throws(()=>describe(()=>h(tag,{lines:2}),{}),/UI_PROP/);
 assert.throws(()=>describe(()=>h('text',{style:{lines:2}},'x'),{}),/UI_STYLE/);
});


test('subtree opacity admits finite unit values and rejects coercion', () => {
  for (const opacity of [0, 0.25, 1]) {
    const buffer = describe(() => h('column', { style: { opacity } }), {});
    assert.deepEqual(buffer.slice(-2), [58, opacity]);
  }
  for (const opacity of [-0.01, 1.01, NaN, Infinity, '0.5', null, true]) {
    assert.throws(() => describe(() => h('column', { style: { opacity } }), {}), /UI_STYLE/);
  }
});

test('composed buttons keep one callback and require explicit accessible names', () => {
  let clicks = 0;
  const render = () => h('button', { accessibilityLabel: 'Open inbox', onClick: () => clicks++ },
    h('row', {}, h('text', {}, 'Inbox'), h('text', {}, '3 unread')));
  const buffer = describe(render, {});
  const button = buttons(buffer)[0];
  assert.equal(button.text, '');
  assert.equal(buttons(buffer).length, 1);
  assert.equal(dispatch(button.handle), true);
  assert.equal(clicks, 1);
  const next = buttons(describe(render, {}))[0];
  assert.equal(dispatch(button.handle), false);
  assert.equal(dispatch(next.handle), true);
  for (const label of [undefined, '', ' ', 1, 'x\0y'])
    assert.throws(() => describe(() => h('button', { accessibilityLabel: label }, h('text', {}, 'Label')), {}), /UI_BUTTON/);
  assert.throws(() => describe(() => h('button', { accessibilityLabel: 'Mixed' }, 'plain', h('text', {}, 'child')), {}), /UI_BUTTON/);
});

test('composed buttons reject interactive descendants through components and containers', () => {
  for (const tag of ['button', 'input', 'textarea', 'search', 'checkbox', 'switch', 'radio', 'slider', 'plot', 'scroll', 'list']) {
    function Nested() { return h('column', {}, h(tag, {})); }
    assert.throws(() => describe(() => h('button', { accessibilityLabel: 'Outer' }, h(Nested, {})), {}), /UI_BUTTON/);
  }
  const buffer = describe(() => h('button', { enabled: false, accessibilityLabel: 'Disabled', onClick() { throw Error('must not run'); } }, h('text', {}, 'Disabled')), {});
  assert.equal(buttons(buffer)[0].handle, undefined);
});

 test('composed checkboxes preserve boolean callbacks and reject nested controls', () => {
  let checked;
  const render = () => h('checkbox', {accessibilityLabel:'Task', value:false, onChange:value=>checked=value}, h('row',{},h('text',{},'Task')));
  const buffer=describe(render,{});
  const props=new Map(); for(let i=6;i<6+buffer[5]*2;i+=2) props.set(buffer[i],buffer[i+1]);
  assert.equal(buffer[0],10); assert.equal(buffer[4],''); assert.equal(props.get(41),'Task');
  assert.equal(dispatch(props.get(40)),true); assert.equal(checked,true);
  for(const label of [undefined,'',' ',1,'x\0y'])
    assert.throws(()=>describe(()=>h('checkbox',{accessibilityLabel:label},h('text',{},'Task')),{}),/UI_BUTTON/);
  for(const tag of ['button','checkbox','input','switch','scroll'])
    assert.throws(()=>describe(()=>h('checkbox',{accessibilityLabel:'Task'},h('column',{},h(tag,{}))),{}),/UI_BUTTON/);
  assert.throws(()=>describe(()=>h('switch',{},h('text',{},'Task')),{}),/UI_CHILD/);
 });

test('vector dash patterns encode bounded positive lengths', () => {
  const path={d:'M0 12H24',fill:'none',stroke:'#000000',strokeDasharray:[4,2,1]};
  const wire=describe(()=>h('vector',{paths:[path]}),{});
  assert.deepEqual(JSON.parse(wire[7]).paths[0].strokeDasharray,[4,2,1]);
  for(const strokeDasharray of [[0,2],[-1], [1/128], [1025], [Infinity], [NaN], '4 2', Array(17).fill(1)])
    assert.throws(()=>describe(()=>h('vector',{paths:[{...path,strokeDasharray}]}),{}),/UI_VECTOR/);
  assert.doesNotThrow(()=>describe(()=>h('vector',{paths:[{...path,strokeDasharray:[]}]}),{}));
});

test('vector fill fitting and fixed strokes remain explicit bounded properties', () => {
  const paths=[{d:'M0 .5H1',fill:'none',stroke:'#000000',vectorEffect:'non-scaling-stroke'}];
  const wire=describe(()=>h('vector',{fit:'fill',paths}),{});
  assert.equal(JSON.parse(wire[7]).fit,'fill');
  assert.equal(JSON.parse(wire[7]).paths[0].vectorEffect,'non-scaling-stroke');
  assert.throws(()=>describe(()=>h('vector',{fit:'cover',paths}),{}),/UI_VECTOR/);
  assert.throws(()=>describe(()=>h('vector',{paths:[{...paths[0],vectorEffect:'invalid'}]}),{}),/UI_VECTOR/);
});


test('backdrop style values are bounded before native submission', () => {
  for (const [name,id,max] of [['backdropBlur',95,64],['backdropSaturation',96,16]]) {
    assert.deepEqual(describe(()=>h('column',{style:{[name]:max}}),{}).slice(6),[id,max]);
    for(const value of [-1,max+1,NaN,Infinity,null,'4px'])
      assert.throws(()=>describe(()=>h('column',{style:{[name]:value}}),{}),/UI_STYLE_VALUE/);
  }
});


test('placeholder color has its own style field', () => {
  assert.deepEqual(describe(()=>h('input',{style:{placeholderColor:'#12345680'}}),{}).slice(6),[97,'#12345680',21,'']);
  for(const value of [null,1,true,{}]) assert.throws(()=>describe(()=>h('input',{style:{placeholderColor:value}}),{}),/UI_STYLE_VALUE/);
});
