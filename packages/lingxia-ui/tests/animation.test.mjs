import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {compile} from 'tailwindcss';
import postcss from 'postcss';
import {lowerAnimation} from '../compiler/animation.mjs';
import {compileUtilities} from '../compiler/tailwind.mjs';
import {configureClasses, resolveClasses} from '../src/style/classes.js';
import {describe, h} from '../src/internal.js';

const tokens = await readFile(new URL('./fixtures/animation-tokens.css',import.meta.url),'utf8');
const fixture = JSON.parse(await readFile(new URL('./fixtures/animations.json',import.meta.url),'utf8'));
const lower = (css,value='enter 350ms linear') => lowerAnimation(value,postcss.parse(css));
const fade = '@keyframes enter {from {opacity:0} to {opacity:1}}';
const theme = `@theme { --animate-enter: enter 200ms linear; @keyframes enter {from {opacity:var(--start)} to {opacity:1}} }
:root {--start:0} [data-theme="dark"] {--start:0.25}`;
const experimental = {animationLowering:true};

test('utility lowering carries exact showcase effects through the existing rule cascade', async () => {
  const rules = await compileUtilities(new Set(['animate-blink','animate-chart-in']),tokens,experimental);
  assert.ok(rules.length >= 2);
  assert.ok(rules.every(rule=>!rule.error),JSON.stringify(rules));
  configureClasses(rules);
  for (const theme of ['light','dark']) for (const name of ['blink','chart-in']) {
    assert.deepEqual(resolveClasses(`animate-${name}`,{theme}).base.animation,fixture[name]);
  }
});

test('animation shorthand reset obeys Tailwind order, viewport and theme rather than class string order', async () => {
  const names = new Set(['animate-enter','animate-none','md:animate-none','dark:animate-none']);
  configureClasses(await compileUtilities(names,theme,experimental));
  const style = (name,theme='light',width=400) => resolveClasses(name,{theme,viewport:{width}}).base.animation;
  assert.equal(style('animate-enter animate-none'),null);
  assert.equal(style('animate-none animate-enter'),null);
  assert.equal(style('animate-enter').opacity[0].value,0);
  assert.equal(style('animate-enter','dark').opacity[0].value,0.25);
  assert.equal(style('animate-enter md:animate-none','light',800),null);
  assert.equal(style('animate-enter md:animate-none','light',400).durationMs,200);
  assert.equal(style('animate-enter dark:animate-none','dark'),null);
  assert.equal(style('animate-enter dark:animate-none','light').durationMs,200);
});

test('nested animation tracks are isolated across rule configuration, cache hits and environment changes', async () => {
  const rules = await compileUtilities(new Set(['animate-enter']),theme,experimental);
  configureClasses(rules);
  rules[0].style.animation.opacity[0].value = 0.9;
  const first = resolveClasses('animate-enter',{theme:'light'}).base.animation;
  first.opacity[0].value = 0.8;
  assert.equal(resolveClasses('animate-enter',{theme:'light'}).base.animation.opacity[0].value,0);
  assert.equal(resolveClasses('animate-enter',{theme:'dark'}).base.animation.opacity[0].value,0.25);
  assert.equal(resolveClasses('animate-enter',{theme:'light'}).base.animation.opacity[0].value,0);
});

test('invalid effects and focus animation report diagnostics while page builds stay gated', async () => {
  const invalid = await compileUtilities(new Set(['animate-enter']), '@theme {--animate-enter:enter 1s;@keyframes enter{from{width:0px}to{width:10px}}}',experimental);
  assert.match(invalid[0].error.message,/unsupported animated property width/);
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

test('official Tailwind output from animation tokens lowers to native fixtures', async () => {
  const defaults = await readFile(new URL(import.meta.resolve('tailwindcss/theme.css')),'utf8');
  const compiler = await compile(`${defaults}\n${tokens}\n@tailwind utilities;`);
  const root = postcss.parse(compiler.build(['animate-blink','animate-chart-in']));
  const vars = new Map();
  root.walkDecls(decl=>{if(decl.prop.startsWith('--'))vars.set(decl.prop,decl.value);});
  const resolve = value => value.replace(/var\((--[-\w]+)\)/g,(_,name)=>vars.get(name)??assert.fail(`missing ${name}`));
  for (const name of ['blink','chart-in']) {
    let value;
    root.walkRules(rule=>{if(rule.selector===`.animate-${name}`)rule.walkDecls('animation',decl=>value=decl.value);});
    assert.ok(value);
    // The rule cascade, not the keyframe lowering, says how reduced motion treats it.
    assert.deepEqual({...lowerAnimation(value,root,resolve),whenReduced:'base'},fixture[name]);
  }
});

test('animations authored under motion-reduce: or motion-safe: are kept under reduced motion', async () => {
  const rules = await compileUtilities(new Set(['animate-spin','motion-reduce:animate-pulse','motion-safe:animate-bounce']),'',experimental);
  const mode = name => rules.find(rule=>rule.name===name).style.animation.whenReduced;
  assert.deepEqual(['animate-spin','motion-reduce:animate-pulse','motion-safe:animate-bounce'].map(mode),['base','keep','keep']);
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
    'from {opacity:2} to {opacity:0}',
    'from {transform:scale(0)} to {transform:scale(1)}',
    'from {transform:scale(1) translateY(2px)} to {transform:translateY(0) scale(1)}',
    'from {opacity:0!important} to {opacity:1}',
    'from {transform:translateY(2%) translateX(1px)} to {transform:translateY(0) translateX(1%)}']) {
    assert.throws(()=>lower(`@keyframes enter {${body}}`),/UI_ANIMATION/,body);
  }
  assert.throws(()=>lower('@media (min-width:1px){'+fade+'}'),/conditional keyframes/);
  assert.throws(()=>lower(`@keyframes enter {${Array.from({length:65},(_,i)=>`${i*100/64}%{opacity:0}`).join('')}}`),/budget/);
});

test('production utility compilation still rejects animations until scheduling is connected', async () => {
  const rules = await compileUtilities(new Set(['animate-pulse']));
  assert.ok(rules.some(rule=>rule.name==='animate-pulse'&&rule.error));
});

test('default Tailwind animations lower with implicit endpoints, rotation, relative translation and per-key easing', async () => {
  const rules = await compileUtilities(new Set(['animate-spin','animate-ping','animate-pulse','animate-bounce']),'',experimental);
  assert.deepEqual(rules.filter(rule=>rule.error),[]);
  configureClasses(rules);
  const effect = name => resolveClasses(`animate-${name}`).base.animation;
  const spin = effect('spin');
  assert.deepEqual([spin.iterations, spin.easing, spin.transforms], [null, 'linear', [{kind:'rotate',angle:[{offset:0,value:0},{offset:1,value:360}]}]]);
  const ping = effect('ping');
  assert.deepEqual(ping.opacity.map(key=>[key.offset,key.value]), [[0,1],[0.75,0],[1,0]]);
  assert.deepEqual(ping.transforms[0].x.map(key=>key.value), [1,2,2]);
  assert.deepEqual(effect('pulse').opacity.map(key=>[key.offset,key.value]), [[0,1],[0.5,0.5],[1,1]]);
  const bounce = effect('bounce').transforms[0];
  assert.equal(bounce.relative, true);
  assert.deepEqual(bounce.y.map(key=>[key.offset,key.value,key.easing]), [[0,-0.25,[0.8,0,1,1]],[0.5,0,[0,0,0.2,1]],[1,-0.25,undefined]]);
});

test('transition utilities lower to one bounded transition of painted properties, behind the motion gate', async () => {
  const names = ['transition', 'transition-all', 'transition-colors', 'transition-opacity', 'transition-transform', 'transition-none', 'duration-200',
    'delay-150', 'ease-in', 'ease-[cubic-bezier(0.1,0.2,0.3,1)]', 'transition-[height]', 'duration-[12s]', 'motion-reduce:transition-none', 'motion-safe:animate-spin'];
  assert.ok((await compileUtilities(new Set(['transition-opacity']))).every(rule => /UI_ANIMATION/.test(rule.error?.message)));
  configureClasses(await compileUtilities(new Set(names), '', experimental));
  const transition = (classes, env) => resolveClasses(classes, env).base.transition;
  assert.deepEqual(transition('transition-opacity duration-200 ease-in delay-150'), { properties: ['opacity'], durationMs: 200, delayMs: 150, easing: [0.4, 0, 1, 1] });
  assert.deepEqual(transition('duration-200 transition-opacity'), transition('transition-opacity duration-200'));
  assert.deepEqual(transition('transition-transform').properties, ['transform']);
  assert.deepEqual(transition('transition-colors').properties, ['fg', 'bg', 'border']);
  assert.deepEqual(transition('transition-all').properties, ['opacity', 'transform', 'bg', 'fg', 'border', 'shadow']);
  assert.deepEqual(transition('transition').durationMs, 150);
  assert.deepEqual(transition('transition-opacity ease-[cubic-bezier(0.1,0.2,0.3,1)]').easing, [0.1, 0.2, 0.3, 1]);
  assert.equal(transition('transition-opacity transition-none'), 'none');
  assert.equal(transition('transition-opacity motion-reduce:transition-none', { reducedMotion: true }), 'none');
  assert.deepEqual(transition('transition-opacity motion-reduce:transition-none', { reducedMotion: false }).properties, ['opacity']);
  assert.equal(resolveClasses('motion-safe:animate-spin', { reducedMotion: true }).base.animation, undefined);
  assert.throws(() => resolveClasses('transition-[height]'), /animates layout; nearest supported: transition-transform/);
  assert.throws(() => resolveClasses('duration-[12s]'), /over 10000ms/);
  const records = describe(() => h('column', { className: 'transition-opacity duration-200' }));
  assert.equal(records[6], 171);
  assert.deepEqual(JSON.parse(records[7]), { properties: ['opacity'], durationMs: 200, delayMs: 0, easing: [0.4, 0, 0.2, 1] });
});
