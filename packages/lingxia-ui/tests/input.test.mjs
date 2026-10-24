import {test} from 'node:test';
import assert from 'node:assert/strict';
import {h,describe,prepare,discard,dispatch,useLocal,reset} from '../src/runtime.js';
import {inputProps} from '../src/schema.js';
function records(buffer) {
  const result=[];
  for(let i=0;i<buffer.length;){const n=buffer[i+5];result.push({tag:buffer[i],text:buffer[i+4],fields:new Map(Array.from({length:n},(_,j)=>buffer.slice(i+6+j*2,i+8+j*2)))});i+=6+n*2;}
  return result;
}
test('editor labels are distinct from values and placeholder hints',()=>{
 for(const tag of ['input','textarea','search']) {
  const fields=records(describe(()=>h(tag,{value:'Ada',placeholder:'Your name',accessibilityLabel:'Name'}),{}))[0].fields;
  assert.equal(fields.get(41),'Name');assert.equal(fields.get(21),'Ada');assert.equal(fields.get(22),'Your name');
  assert.throws(()=>describe(()=>h(tag,{accessibilityLabel:'  '}),{}),/UI_INPUT/);
 }
});

test('local string bindings receive only typed committed text events',()=>{
  reset();
  const render=()=>{const draft=useLocal('draft','');return h('column',{},h('input',{bind:draft}),h('text',{},draft.value));};
  const first=records(describe(render,{})),handle=first[1].fields.get(inputProps.onInput);
  assert.equal(dispatch(handle),false);
  assert.equal(dispatch(handle,42),false);
  assert.equal(dispatch(handle,'你好😀\n"quoted"'),true);
  const next=records(describe(render,{}));
  assert.equal(next[1].fields.get(inputProps.value),'你好😀\n"quoted"');
  assert.equal(next[2].text,'你好😀\n"quoted"');
  assert.equal(dispatch(handle,'stale'),false);
});
test('input callbacks survive discarded candidates and disabled inputs register none',()=>{
  reset();let received='';
  const render=()=>h('textarea',{value:'a',onSubmit:value=>received=value});
  const old=records(describe(render,{}))[0].fields.get(inputProps.onSubmit);
  const pending=records(prepare(render,{}))[0].fields.get(inputProps.onSubmit);discard();
  assert.equal(dispatch(pending,'bad'),false);assert.equal(dispatch(old,'ok'),true);assert.equal(received,'ok');
  const disabled=records(describe(()=>h('input',{enabled:false,onInput(){},onSubmit(){}}),{}))[0];
  assert.equal(disabled.fields.has(inputProps.onInput),false);assert.equal(disabled.fields.has(inputProps.onSubmit),false);
});
test('input descriptions validate text, bindings, children and props',()=>{
  for(const props of [{value:null},{value:1},{value:'中'.repeat(22000)},{value:'\uD800'},{placeholder:false},{secure:1},{autoFocus:1},{keyboard:'url'},{onInput:1},{bind:{value:'x',set(){}}}])
    assert.throws(()=>describe(()=>h('input',props),{}),/UI_INPUT/);
  assert.throws(()=>describe(()=>h('textarea',{},'child'),{}),/UI_CHILD/);
  assert.throws(()=>describe(()=>h('input',{onClick(){}}),{}),/UI_PROP/);
  assert.throws(()=>describe(()=>h('input',{bind:useLocal('bad',42)}),{}),/UI_INPUT/);
  assert.throws(()=>describe(()=>h('input',{bind:useLocal('ambiguous',''),value:'x'}),{}),/UI_INPUT/);
});
test('text event validation rejects oversized and malformed UTF-16 payloads',()=>{
  reset();let count=0;
  const handle=records(describe(()=>h('input',{onInput(){count++;}}),{}))[0].fields.get(inputProps.onInput);
  assert.equal(dispatch(handle,'中'.repeat(22000)),false);
  assert.equal(dispatch(handle,'\uDFFF'),false);
  assert.equal(count,0);
  assert.equal(dispatch(handle,'valid'),true);assert.equal(count,1);
});

test('bind rejects cells removed from the current candidate',()=>{
  reset();let saved;
  describe(()=>{saved=useLocal('draft','old');return h('text',{},saved.value);},{});
  describe(()=>h('text',{},'removed'),{});
  assert.throws(()=>describe(()=>h('input',{bind:saved}),{}),/UI_INPUT/);
});

test('ASCII fast validation preserves the exact UTF-8 byte limit',()=>{
  reset();let received='';
  const handle=records(describe(()=>h('input',{onInput:value=>received=value}),{}))[0].fields.get(inputProps.onInput);
  assert.equal(dispatch(handle,'x'.repeat(65536)),true);assert.equal(received.length,65536);
  assert.equal(dispatch(handle,'x'.repeat(65537)),false);assert.equal(received.length,65536);
});


test('search binds committed text and submits through a search keyboard',()=>{
  reset();let submitted='';
  const render=()=>h('search',{bind:useLocal('query',''),onSubmit:value=>submitted=value});
  const first=records(describe(render,{}))[0];
  assert.equal(first.tag,14);assert.equal(first.fields.get(inputProps.keyboard),'search');
  assert.equal(dispatch(first.fields.get(inputProps.onInput),'coffee'),true);
  const next=records(describe(render,{}))[0];assert.equal(next.fields.get(inputProps.value),'coffee');
  assert.equal(dispatch(next.fields.get(inputProps.onSubmit),'coffee'),true);assert.equal(submitted,'coffee');
  for(const props of [{secure:true},{secure:false},{keyboard:'text'},{keyboard:'search'}])
    assert.throws(()=>describe(()=>h('search',props),{}),/UI_INPUT/);
  assert.throws(()=>describe(()=>h('search',{},'child'),{}),/UI_CHILD/);
  const disabled=records(describe(()=>h('search',{enabled:false,onInput(){},onSubmit(){}}),{}))[0];
  assert.equal(disabled.fields.has(inputProps.onInput),false);assert.equal(disabled.fields.has(inputProps.onSubmit),false);
});

test('auto height is explicit, while unsupported auto dimensions are rejected',()=>{
  reset();
  const record=records(describe(()=>h('textarea',{style:{height:'auto'}}),{}))[0];
  assert.equal(record.fields.get(82),1);
  assert.throws(()=>describe(()=>h('textarea',{style:{maxHeight:'auto'}}),{}),/UI_STYLE_VALUE/);
});

test('submitOnEnter is an explicit textarea flag with a normal submit callback',()=>{
  reset(); let sent;
  const record=records(describe(()=>h('textarea',{submitOnEnter:true,onSubmit:v=>sent=v}),{}))[0];
  assert.equal(record.fields.get(inputProps.submitOnEnter),1);
  assert.equal(dispatch(record.fields.get(inputProps.onSubmit),'message'),true);
  assert.equal(sent,'message');
  for(const tag of ['input','search']) assert.throws(()=>describe(()=>h(tag,{submitOnEnter:true}),{}),/UI_INPUT/);
  for(const value of [1,'true',null]) assert.throws(()=>describe(()=>h('textarea',{submitOnEnter:value}),{}),/UI_INPUT/);
});
