import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {compile} from 'tailwindcss';
import postcss from 'postcss';
import {lowerAnimation} from '../animation.mjs';
import {compileUtilities} from '../tailwind.mjs';
import {configureClasses, resolveStyle} from '../src/classes.js';
import {describe, h} from '../src/runtime.js';

const fixture = JSON.parse(await readFile(new URL('./fixtures/animations.json',import.meta.url),'utf8'));
const lower = (css,value='enter 350ms linear') => lowerAnimation(value,postcss.parse(css));
const fade = '@keyframes enter {from {opacity:0} to {opacity:1}}';
const theme = `@theme { --animate-enter: enter 200ms linear; @keyframes enter {from {opacity:var(--start)} to {opacity:1}} }
:root {--start:0} [data-theme="dark"] {--start:0.25}`;
const experimental = {animationLowering:true};

test('utility lowering carries exact showcase effects through the existing rule cascade', async () => {
  const tokens = await readFile(new URL('../../../examples/lingxia-showcase/lxapp/theme/tokens.css',import.meta.url),'utf8');
  // Browser pre-stamp media fallbacks are separate from native View.env themes.
  const animationTheme = postcss.parse(tokens).nodes.find(node=>node.type==='atrule'&&node.name==='theme'&&node.nodes.some(child=>child.prop==='--animate-blink'));
  const rules = await compileUtilities(new Set(['animate-blink','animate-chart-in']),animationTheme.toString(),experimental);
  assert.ok(rules.length >= 2);
  assert.ok(rules.every(rule=>!rule.error),JSON.stringify(rules));
  configureClasses(rules);
  for (const theme of ['light','dark']) for (const name of ['blink','chart-in']) {
    assert.deepEqual(resolveStyle(`animate-${name}`,undefined,{theme}).style.animation,fixture[name]);
  }
});

test('animation shorthand reset obeys Tailwind order, viewport and theme rather than class string order', async () => {
  const names = new Set(['animate-enter','animate-none','md:animate-none','dark:animate-none']);
  configureClasses(await compileUtilities(names,theme,experimental));
  const style = (name,theme='light',width=400) => resolveStyle(name,undefined,{theme,viewport:{width}}).style.animation;
  assert.equal(style('animate-enter animate-none'),null);
  assert.equal(style('animate-none animate-enter'),null);
  assert.equal(style('animate-enter').opacity[0].value,0);
  assert.equal(style('animate-enter','dark').opacity[0].value,0.25);
  assert.equal(style('animate-enter md:animate-none','light',800),null);
  assert.equal(style('animate-enter md:animate-none','light',400).durationMs,200);
  assert.equal(style('animate-enter dark:animate-none','dark'),null);
  assert.equal(style('animate-enter dark:animate-none','light').durationMs,200);
  assert.equal(resolveStyle('animate-enter',{animation:null},{theme:'light'}).style.animation,null);
});

test('nested animation tracks are isolated across rule configuration, cache hits and environment changes', async () => {
  const rules = await compileUtilities(new Set(['animate-enter']),theme,experimental);
  configureClasses(rules);
  rules[0].style.animation.opacity[0].value = 0.9;
  const first = resolveStyle('animate-enter',undefined,{theme:'light'}).style.animation;
  first.opacity[0].value = 0.8;
  assert.equal(resolveStyle('animate-enter',undefined,{theme:'light'}).style.animation.opacity[0].value,0);
  assert.equal(resolveStyle('animate-enter',undefined,{theme:'dark'}).style.animation.opacity[0].value,0.25);
  assert.equal(resolveStyle('animate-enter',undefined,{theme:'light'}).style.animation.opacity[0].value,0);
});

test('invalid effects and focus animation report diagnostics while page builds stay gated', async () => {
  const invalid = await compileUtilities(new Set(['animate-enter']), '@theme {--animate-enter:enter 1s;@keyframes enter{from{width:0px}to{width:10px}}}',experimental);
  assert.match(invalid[0].error,/unsupported animated property width/);
  const focus = await compileUtilities(new Set(['focus:animate-enter']),theme,experimental);
  assert.match(focus[0].error,/focus supports paint properties only/);
  configureClasses(await compileUtilities(new Set(['animate-enter','animate-none']),theme,experimental));
  const records = describe(()=>h('column',{className:'animate-enter'}));
  assert.equal(records[6],101);
  assert.equal(JSON.parse(records[7]).name,'enter');
  assert.equal(describe(()=>h('column',{className:'animate-none'}))[5],0);
  assert.equal(describe(()=>h('column',{className:'animate-enter',style:{animation:null}}))[5],0);
  await assert.rejects(()=>compileUtilities(new Set(['animate-enter']),theme),/UI_ANIMATION/);
});

test('animation wire preserves exact payload and bounds serialization before transport', () => {
  const encoded = describe(()=>h('column',{style:{animation:fixture.blink}}));
  assert.equal(encoded[6],101);
  assert.deepEqual(JSON.parse(encoded[7]),fixture.blink);
  for (const value of [false,[],42,'blink', {value:NaN},{value:Infinity},{value:undefined},{value:()=>{}},{value:'x'.repeat(32768)}]) {
    assert.throws(()=>describe(()=>h('column',{style:{animation:value}})),/UI_ANIMATION/);
  }
  const circular={}; circular.self=circular;
  assert.throws(()=>describe(()=>h('column',{style:{animation:circular}})),/circular|cyclic/i);
  const payload = {...fixture.blink,padding:'x'.repeat(32000)};
  assert.throws(()=>describe(()=>h('column',{},Array.from({length:9},(_,i)=>h('column',{key:String(i),style:{animation:payload}})))),/page payload budget/);
});

test('animation transport budgets count UTF-8 bytes at definition and page boundaries', () => {
  // Transport validates size independently of the native definition schema.
  const base = {...fixture.blink,padding:''};
  const overhead = Buffer.byteLength(JSON.stringify(base));
  for (const [character, bytes] of [['x',1],['é',2],['图',3],['🙂',4]]) {
    const count = Math.floor((32768 - overhead) / bytes);
    const padding = character.repeat(count) + 'x'.repeat((32768 - overhead) % bytes);
    const payload = {...base,padding};
    assert.equal(Buffer.byteLength(JSON.stringify(payload)),32768);
    const render = n => describe(()=>h('column',{},Array.from({length:n},(_,i)=>h('column',{key:String(i),style:{animation:payload}}))));
    assert.doesNotThrow(()=>render(8));
    assert.throws(()=>render(9),/page payload budget/);
    assert.throws(()=>describe(()=>h('column',{style:{animation:{...payload,padding:padding+'x'}}})),/wire budget/);
  }
});

test('official Tailwind output from React showcase tokens lowers to native fixtures', async () => {
  const defaults = await readFile(new URL(import.meta.resolve('tailwindcss/theme.css')),'utf8');
  const theme = await readFile(new URL('../../../examples/lingxia-showcase/lxapp/theme/tokens.css',import.meta.url),'utf8');
  const compiler = await compile(`${defaults}\n${theme}\n@tailwind utilities;`);
  const root = postcss.parse(compiler.build(['animate-blink','animate-chart-in']));
  const vars = new Map();
  root.walkDecls(decl=>{if(decl.prop.startsWith('--'))vars.set(decl.prop,decl.value);});
  const resolve = value => value.replace(/var\((--[-\w]+)\)/g,(_,name)=>vars.get(name)??assert.fail(`missing ${name}`));
  for (const name of ['blink','chart-in']) {
    let value;
    root.walkRules(rule=>{if(rule.selector===`.animate-${name}`)rule.walkDecls('animation',decl=>value=decl.value);});
    assert.ok(value);
    assert.deepEqual(lowerAnimation(value,root,resolve),fixture[name]);
  }
});

test('keyframe source order merges offsets and later complete definitions replace earlier ones', () => {
  const css = '@keyframes enter {from,50% {opacity:0} 50% {opacity:0.4} to {opacity:1}}';
  assert.deepEqual(lower(css).opacity,[{offset:0,value:0},{offset:.5,value:.4},{offset:1,value:1}]);
  assert.deepEqual(lower(`${css} ${fade}`).opacity,fixture['chart-in'].opacity);
  assert.equal(lower(fade,'none'),null);
  const timing = lower(fade,'2 enter linear 0.35s 12ms');
  assert.equal(timing.durationMs,350); assert.equal(timing.delayMs,12); assert.equal(timing.iterations,2);
});

test('equal tracks retain distinct animation names for native restart identity', () => {
  const css = fade + fade.replace('enter','replacement');
  const first = lower(css), second = lower(css,'replacement 350ms linear');
  assert.deepEqual(first.opacity,second.opacity);
  assert.equal(first.name,'enter'); assert.equal(second.name,'replacement');
  assert.throws(()=>lower(css,`${'a'.repeat(129)} 1s`),/invalid name/);
});

test('transform order, identity endpoints, axes and unit conversion survive lowering', () => {
  const effect = lower('@keyframes enter {from {transform:scale(.96) translateY(.5rem)} to {transform:none}}');
  assert.deepEqual(effect.transforms,fixture['chart-in'].transforms);
  const axes = lower('@keyframes enter {from {transform:scaleX(2) translate(2px,3px) scaleY(3)} to {transform:scaleX(1) translate(0,0) scaleY(1)}}');
  assert.deepEqual(axes.transforms.map(t=>t.kind),['scale','translate','scale']);
  assert.deepEqual(axes.transforms[0].y.map(k=>k.value),[1,1]);
  assert.deepEqual(axes.transforms[2].x.map(k=>k.value),[1,1]);
});

test('unsupported modes and unbounded effects fail explicitly', () => {
  for (const value of ['enter 0ms','enter -1s','enter 1s -1s','enter 86401s','enter 1s 1.5',
    'enter 1s reverse','enter 1s forwards','enter 1s paused','enter 1s, enter 2s',
    'enter 1s steps(2)','enter 1s cubic-bezier(2,0,1,1)','enter 1s 4294967296']) {
    assert.throws(()=>lower(fade,value),/UI_ANIMATION/,value);
  }
  for (const body of ['from {width:10px} to {width:20px}',
    '50% {opacity:0} to {opacity:1}', 'from {opacity:2} to {opacity:0}',
    'from {transform:scale(0)} to {transform:scale(1)}',
    'from {transform:rotate(10deg)} to {transform:rotate(0deg)}',
    'from {transform:translateY(2%)} to {transform:translateY(0)}',
    'from {transform:scale(1) translateY(2px)} to {transform:translateY(0) scale(1)}',
    'from {opacity:0!important} to {opacity:1}',
    'from {animation-timing-function:linear;opacity:0} to {opacity:1}']) {
    assert.throws(()=>lower(`@keyframes enter {${body}}`),/UI_ANIMATION/,body);
  }
  assert.throws(()=>lower('@media (min-width:1px){'+fade+'}'),/conditional keyframes/);
  assert.throws(()=>lower(`@keyframes enter {${Array.from({length:65},(_,i)=>`${i*100/64}%{opacity:0}`).join('')}}`),/budget/);
});

test('production utility compilation still rejects animations until scheduling is connected', async () => {
  const rules = await compileUtilities(new Set(['animate-pulse']));
  assert.ok(rules.some(rule=>rule.name==='animate-pulse'&&rule.error));
});
