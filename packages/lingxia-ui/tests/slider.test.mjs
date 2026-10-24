import {test} from 'node:test';
import assert from 'node:assert/strict';
import {h,describe,prepare,discard,dispatch,useLocal,reset} from '../src/runtime.js';
import {sliderProps} from '../src/schema.js';
const fields=buffer=>new Map(Array.from({length:buffer[5]},(_,i)=>buffer.slice(6+i*2,8+i*2)));

test('slider clamps and snaps numeric proposals including a partial final interval',()=>{
 reset();let calls=0;
 const render=()=>{const value=useLocal('value',0);return h('slider',{accessibilityLabel:'Volume',min:0,max:10,step:3,value:value.value,onChange:next=>{calls++;value.set(next);}});};
 let output=fields(describe(render,{}));assert.equal(output.get(sliderProps.value),0);
 for(const [proposal,expected] of [[4,3],[8,9],[9.6,10],[20,10],[-1,0]]) {
   const handle=output.get(sliderProps.onChange);assert.equal(dispatch(handle,proposal),true);
   output=fields(describe(render,{}));assert.equal(output.get(sliderProps.value),expected);
   assert.equal(dispatch(handle,1),false);
 }
 assert.equal(calls,4);
 const handle=output.get(sliderProps.onChange);
 for(const value of [undefined,'3',true,NaN,Infinity,1e10]) assert.equal(dispatch(handle,value),false);
 const pending=fields(prepare(render,{})).get(sliderProps.onChange);discard();assert.equal(dispatch(pending,3),false);assert.equal(dispatch(handle,3),true);
});

test('slider validates bounds, finite numeric binding, labels and disabled callbacks',()=>{
 reset();
 const render=()=>h('slider',{accessibilityLabel:'Intensity',bind:useLocal('intensity',0.5),min:-1,max:1,step:0.25});
 let output=fields(describe(render,{}));assert.equal(dispatch(output.get(sliderProps.onChange),-0.6),true);
 output=fields(describe(render,{}));assert.equal(output.get(sliderProps.value),-0.5);
 for(const props of [{value:null},{value:'1'},{value:NaN},{min:null},{max:0},{step:0},{step:101},{step:0.00000001},{min:1e9,max:1e9+1},{accessibilityLabel:''},{onChange:1},{bind:{value:1,set(){}}}])
  assert.throws(()=>describe(()=>h('slider',{accessibilityLabel:'Range',...props}),{}),/UI_SLIDER/);
 assert.throws(()=>describe(()=>h('slider',{accessibilityLabel:'Range',bind:useLocal('bad','1')}),{}),/UI_SLIDER/);
 assert.throws(()=>describe(()=>h('slider',{accessibilityLabel:'Range'},'label'),{}),/UI_CHILD/);
 output=fields(describe(()=>h('slider',{accessibilityLabel:'Range',enabled:false,onChange(){}}),{}));
 assert.equal(output.has(sliderProps.onChange),false);
});
