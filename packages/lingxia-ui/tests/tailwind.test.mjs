import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compileUtilities } from '../tailwind.mjs';
import { configureClasses, resolveStyle, focusStyle } from '../src/classes.js';
import { describe, h } from '../src/runtime.js';
const corners = radius => ({radiusTopLeft:radius,radiusTopRight:radius,radiusBottomRight:radius,radiusBottomLeft:radius});
const classStyle = (name, env) => resolveStyle(name, undefined, env).style;

test('flex direction follows utility cascade, responsive rules and explicit style', async () => {
  configureClasses(await compileUtilities(new Set(['flex','flex-col','md:flex-row','flex-row-reverse','flex-col-reverse'])));
  assert.deepEqual(classStyle('flex flex-col md:flex-row', {viewport:{width:360}}), {display:'flex',flexDirection:'column'});
  assert.deepEqual(classStyle('md:flex-row flex-col flex', {viewport:{width:900}}), {display:'flex',flexDirection:'row'});
  assert.equal(resolveStyle('flex-col',{flexDirection:'row-reverse'}).style.flexDirection,'row-reverse');
  assert.equal(classStyle('flex-col-reverse').flexDirection,'column-reverse');
  const records = describe(() => h('view',{className:'flex flex-col'},h('text',{},'Hello')));
  assert.equal(records[0],1);
  assert.ok(records.includes(103) && records.includes('column') && records.includes(104));
  assert.throws(() => describe(() => h('text',{className:'flex'},'Hello')), /UI_LAYOUT/);
  assert.throws(() => describe(() => h('grid',{columns:2,className:'flex-col'})), /UI_LAYOUT/);
});

test('horizontal overflow lowers only onto scroll containers', async () => {
  configureClasses(await compileUtilities(new Set(['overflow-x-auto','md:overflow-x-auto'])));
  assert.deepEqual(classStyle('overflow-x-auto'), {scrollAxis:'horizontal'});
  assert.deepEqual(classStyle('md:overflow-x-auto',{viewport:{width:320}}), {});
  assert.deepEqual(classStyle('md:overflow-x-auto',{viewport:{width:900}}), {scrollAxis:'horizontal'});
  const encoded = describe(() => h('scroll',{className:'overflow-x-auto'},h('row',{},h('text',{},'Symbols'))));
  assert.equal(encoded[encoded.indexOf(100)+1],'horizontal');
  assert.throws(() => describe(() => h('row',{className:'overflow-x-auto'})), /scrollAxis requires a scroll node/);
  assert.throws(() => describe(() => h('scroll',{style:{scrollAxis:'both'}})), /UI_STYLE_VALUE/);
  assert.equal(resolveStyle('overflow-x-auto',{scrollAxis:'vertical'}).style.scrollAxis,'vertical');
});

test('baseline utilities preserve first-baseline alignment through the runtime', async () => {
  configureClasses(await compileUtilities(new Set(['items-baseline','self-baseline'])));
  assert.deepEqual(classStyle('items-baseline self-baseline'), {align:'baseline',alignSelf:'baseline'});
  assert.doesNotThrow(() => describe(() => h('row', {className:'items-baseline'},
    h('text', {className:'self-baseline'}, 'Price'))));
});

test('cached class cascades isolate mutations, typography, focus and environments', async () => {
  const rules = await compileUtilities(new Set(['p-4','leading-relaxed','tracking-wider','shadow-sm','text-red-500','dark:text-white','md:p-8','focus:text-blue-500']));
  configureClasses(rules);
  const classes='p-4 leading-relaxed tracking-wider shadow-sm text-red-500 dark:text-white md:p-8 focus:text-blue-500';
  const light={theme:'light',viewport:{width:400}};
  const first=resolveStyle(classes,undefined,light,{fontSize:16}).style;
  first.boxShadow[0].color='#123456'; first.paddingTop=900;
  const second=resolveStyle(classes,{fontSize:20},light).style;
  assert.equal(second.paddingTop,16); assert.equal(second.lineHeight,32.5); assert.equal(second.letterSpacing,1);
  assert.notEqual(second.boxShadow[0].color,'#123456');
  assert.notEqual(focusStyle(classes,undefined,light,{fontSize:16}).color,second.color);
  const dark=resolveStyle(classes,undefined,{theme:'dark',viewport:{width:900}}).style;
  assert.equal(dark.paddingTop,32); assert.notEqual(dark.color,second.color);
  assert.equal(resolveStyle(classes,undefined,light).style.paddingTop,16);
  configureClasses([]); assert.throws(()=>classStyle(classes),/was not compiled/);
});

test('flex growth, basis and shorthands retain independent cascade values', async () => {
  configureClasses(await compileUtilities(new Set(['grow','basis-0','basis-auto','basis-1/2','basis-12','flex-1','flex-auto','flex-none','flex-initial','md:basis-12'])));
  assert.deepEqual(classStyle('grow'), {grow:1});
  assert.deepEqual(classStyle('grow basis-0'), {grow:1,flexBasis:0});
  assert.deepEqual(classStyle('flex-1'), {grow:1,shrink:1,flexBasis:'0%'});
  assert.deepEqual(classStyle('flex-auto'), {grow:1,shrink:1,flexBasis:'auto'});
  assert.deepEqual(classStyle('flex-none'), {grow:0,shrink:0,flexBasis:'auto'});
  assert.deepEqual(classStyle('flex-initial'), {grow:0,shrink:1,flexBasis:'auto'});
  assert.equal(classStyle('basis-1/2').flexBasis,'50%');
  assert.equal(classStyle('basis-1/2 md:basis-12',{viewport:{width:900}}).flexBasis,48);
  assert.equal(classStyle('flex-1 basis-auto').flexBasis,'auto');
  assert.deepEqual(classStyle('flex-1 basis-auto'),classStyle('basis-auto flex-1'));
  assert.equal(resolveStyle('basis-12',{flexBasis:'25%'}).style.flexBasis,'25%');
});

test('percentage and fractional dimensions preserve relative layout values through overrides', async () => {
  configureClasses(await compileUtilities(new Set(['w-full','w-1/2','h-1/3','max-w-[72%]','min-h-full','md:w-1/2','gap-[10%]'])));
  const {height,...rest} = classStyle('w-full h-1/3 max-w-[72%] min-h-full');
  assert.deepEqual(rest,{width:'100%',maxWidth:'72%',minHeight:'100%'});
  assert.ok(Math.abs(parseFloat(height)-100/3)<1e-10);
  assert.equal(classStyle('w-full md:w-1/2',{viewport:{width:800}}).width,'50%');
  assert.equal(resolveStyle('w-full',{width:200}).style.width,200);
  assert.equal(resolveStyle('w-1/2',{width:'75%'}).style.width,'75%');
  assert.throws(() => classStyle('gap-[10%]'),/UI_CLASS/);
});

test('outer shadows preserve layers, color overrides, variants and CSS order', async () => {
  const names = ['shadow-sm','shadow-lg','shadow-none','shadow-red-500/40','dark:shadow-blue-500','md:shadow-none'];
  const rules = await compileUtilities(new Set(names));
  assert.deepEqual(rules.filter(rule => rule.error), []);
  configureClasses(rules);
  assert.deepEqual(classStyle('shadow-lg').boxShadow, [
    {offsetX:0,offsetY:10,blur:15,spread:-3,color:'#0000001a'},
    {offsetX:0,offsetY:4,blur:6,spread:-4,color:'#0000001a'},
  ]);
  const classes = 'shadow-sm shadow-red-500/40 dark:shadow-blue-500 md:shadow-none';
  const env = {theme:'light',viewport:{width:400}};
  const style = classStyle(classes,env);
  assert.deepEqual(style,classStyle(classes.split(' ').reverse().join(' '),env));
  assert.equal(style.boxShadow.length,2);
  assert.match(style.boxShadow[0].color,/66$/);
  assert.equal(style.boxShadow[0].color,style.boxShadow[1].color);
  assert.match(classStyle(classes,{...env,theme:'dark'}).boxShadow[0].color,/ff$/);
  assert.equal(classStyle(classes,{...env,viewport:{width:800}}).boxShadow[0].color,'#00000000');
  assert.deepEqual(classStyle('shadow-red-500/40'),{});
  assert.equal(resolveStyle(classes,{boxShadow:'none'},env).style.boxShadow,'none');
});

test('arbitrary outer shadows retain currentColor and reject unsupported or excessive geometry', async () => {
  configureClasses(await compileUtilities(new Set(['shadow-[0_2px_4px]','shadow-[2px_4px_0_3px_#ff0000]','shadow-[0_0_257px_black]','inset-shadow-sm','ring-offset-2','[box-shadow:var(--tw-shadow)]','[box-shadow:1_2_red]'])));
  assert.deepEqual(classStyle('shadow-[0_2px_4px]').boxShadow,[{offsetX:0,offsetY:2,blur:4,spread:0,color:'currentColor'}]);
  assert.deepEqual(classStyle('shadow-[2px_4px_0_3px_#ff0000]').boxShadow,[{offsetX:2,offsetY:4,blur:0,spread:3,color:'#ff0000ff'}]);
  for (const name of ['shadow-[0_0_257px_black]','inset-shadow-sm','ring-offset-2','[box-shadow:var(--tw-shadow)]','[box-shadow:1_2_red]']) assert.throws(() => classStyle(name), /UI_CLASS/);
});

test('standard system font stacks lower to native generic families without accepting custom faces', async () => {
  configureClasses(await compileUtilities(new Set(['font-mono', 'font-sans', 'font-serif', '[font-family:monospace]', '[font-family:Custom,sans-serif]'])));
  assert.deepEqual(classStyle('font-mono'), {fontFamily:'mono'});
  assert.deepEqual(classStyle('font-sans'), {fontFamily:'sans'});
  assert.deepEqual(classStyle('[font-family:monospace]'), {fontFamily:'mono'});
  assert.throws(() => classStyle('font-serif'), /UI_CLASS/);
  assert.throws(() => classStyle('[font-family:Custom,sans-serif]'), /UI_CLASS/);
});

test('gradient utilities compose across classes using Tailwind cascade and theme', async () => {
  const names = ['bg-linear-to-br', 'bg-linear-to-r/srgb', 'from-red-500', 'via-green-500', 'to-blue-500', 'from-80%', 'via-10%', 'to-20%', 'dark:from-white', 'md:bg-none', 'bg-white'];
  const rules = await compileUtilities(new Set(names));
  assert.deepEqual(rules.filter(rule => rule.error), []);
  configureClasses(rules);
  const classes = 'bg-white bg-linear-to-br from-red-500 via-green-500 to-blue-500 from-80% via-10% to-20% dark:from-white md:bg-none';
  const env = {theme:'light',viewport:{width:400}};
  const style = classStyle(classes, env);
  assert.deepEqual(style, classStyle(classes.split(' ').reverse().join(' '), env));
  assert.equal(style.background, '#ffffffff');
  assert.equal(style.backgroundGradient.direction, 'bottom-right');
  assert.equal(style.backgroundGradient.colorSpace, 'oklab');
  assert.deepEqual(style.backgroundGradient.stops.map(stop => stop.offset), [.8,.8,.8]);
  assert.equal(classStyle(classes, {...env,theme:'dark'}).backgroundGradient.stops[0].color, '#ffffffff');
  assert.equal(classStyle(classes, {...env,viewport:{width:800}}).backgroundGradient, 'none');
  assert.equal(classStyle('bg-linear-to-r/srgb from-red-500 to-blue-500').backgroundGradient.colorSpace, 'srgb');
  assert.equal(resolveStyle(classes, {backgroundGradient:'none'}, env).style.backgroundGradient, 'none');
  assert.deepEqual(classStyle('from-red-500'), {});
});

test('gradient defaults, transparency, negative stops and unsupported spaces are explicit', async () => {
  configureClasses(await compileUtilities(new Set(['bg-linear-45', 'from-red-500/50', 'to-blue-500', 'from-[-10%]', 'to-[120%]', 'bg-linear-to-r/hsl', 'via-[12px]'])));
  const gradient = classStyle('bg-linear-45 from-red-500/50 from-[-10%] to-blue-500 to-[120%]').backgroundGradient;
  assert.equal(gradient.angle, 45);
  assert.equal(gradient.stops[0].offset, -.1);
  assert.equal(gradient.stops[1].offset, 1.2);
  assert.match(gradient.stops[0].color, /80$/);
  const single = classStyle('bg-linear-45 from-red-500/50').backgroundGradient;
  assert.equal(single.stops[1].color, '#00000000');
  assert.equal(classStyle('bg-linear-45').backgroundGradient, 'none');
  for (const name of ['bg-linear-to-r/hsl','via-[12px]']) assert.throws(() => classStyle(name), /UI_CLASS/);
});

test('text alignment utilities retain physical and direction-aware values', async () => {
  const values = ['left', 'center', 'right', 'start', 'end', 'justify'];
  configureClasses(await compileUtilities(new Set(values.map(value => `text-${value}`))));
  for (const value of values) assert.deepEqual(classStyle(`text-${value}`), {textAlign:value});
  assert.equal(resolveStyle('text-right', {textAlign:'left'}).style.textAlign, 'left');
  assert.throws(() => describe(() => h('text', {style:{textAlign:'invalid'}}, 'value')), /UI_STYLE_VALUE/);
});

test('infinite Tailwind radius lowers to the native clamped radius budget', async () => {
  configureClasses(await compileUtilities(new Set(['rounded-full', 'rounded-none', '[width:calc(infinity*1px)]'])));
  assert.deepEqual(classStyle('rounded-full'), {radius:16384,...corners(16384)});
  assert.deepEqual(classStyle('rounded-none'), {radius:0,...corners(0)});
  assert.equal(resolveStyle('rounded-full', {radius:8}).style.radius, 8);
  assert.throws(() => classStyle('[width:calc(infinity*1px)]'), /UI_CLASS/);
});

test('real Tailwind utilities lower to native dimensions, typography and colors', async () => {
  configureClasses(await compileUtilities(new Set(['p-6', 'gap-2', 'text-sm', 'font-semibold', 'rounded-xl', 'bg-blue-500/80', 'opacity-50'])));
  assert.deepEqual(classStyle('p-6 gap-2 text-sm font-semibold rounded-xl opacity-50'), {
    paddingTop: 24, paddingRight: 24, paddingBottom: 24, paddingLeft: 24, gap: 8, fontSize: 14, lineHeight: 20, fontWeight: 600, radius: 12, ...corners(12), opacity: .5,
  });
  assert.match(classStyle('bg-blue-500/80').background, /^#[0-9a-f]{6}cc$/);
});

test('cascade follows Tailwind order and responds to viewport and appearance', async () => {
  configureClasses(await compileUtilities(new Set(['gap-2', 'md:gap-4', 'bg-white', 'dark:bg-black', 'p-4', 'p-2'])));
  assert.equal(classStyle('p-4 p-2').paddingLeft, 16);
  assert.equal(classStyle('p-2 p-4').paddingLeft, 16);
  assert.deepEqual(classStyle('gap-2 md:gap-4 bg-white dark:bg-black', { theme: 'light', viewport: { width: 767 } }), { gap: 8, background: '#ffffffff' });
  assert.deepEqual(classStyle('gap-2 md:gap-4 bg-white dark:bg-black', { theme: 'dark', viewport: { width: 768 } }), { gap: 16, background: '#000000ff' });
});

test('custom theme tokens and arbitrary values retain Tailwind semantics', async () => {
  configureClasses(await compileUtilities(new Set(['bg-primary', 'rounded-xl', 'h-[50px]', 'text-[17px]']), '@theme { --color-primary: #007aff; --radius-xl: 16px; }'));
  assert.deepEqual(classStyle('bg-primary rounded-xl h-[50px] text-[17px]'), { background: '#007affff', radius: 16, ...corners(16), height: 50, fontSize: 17 });
});

test('unsupported rendering and missing dynamic candidates fail visibly', async () => {
  configureClasses(await compileUtilities(new Set(['backdrop-invert', 'hover:opacity-50', 'w-auto', 'p-4'])));
  for (const name of ['backdrop-invert', 'hover:opacity-50', 'w-auto', 'p-8']) assert.throws(() => classStyle(name), /UI_CLASS/);
  assert.throws(() => classStyle(false), /className must be a string/);
  assert.deepEqual(classStyle(undefined), {});
  assert.deepEqual(classStyle('p-4'), { paddingTop: 16, paddingRight: 16, paddingBottom: 16, paddingLeft: 16 });
});

test('stylesheets cannot silently discard browser selectors or imports', async () => {
  for (const css of [':root { color: red; }', '@import "tailwindcss";', '.card { padding: 12px; }'])
    await assert.rejects(compileUtilities(new Set(['p-4']), css), /UI_CSS/);
});

test('semantic aliases resolve the active palette before utility cascade', async () => {
  const css = `
    @theme inline { --color-surface: var(--lx-surface); --color-foreground: var(--lx-text); --color-primary: var(--lx-primary); }
    :root { color-scheme: light; --lx-surface: #ffffff; --lx-gray-900: #1c1c1e; --lx-text: var(--lx-gray-900); --lx-primary: #007aff; }
    [data-theme='dark'] { color-scheme: dark; --lx-surface: #1f1f22; --lx-gray-900: #f2f2f7; --lx-primary: #0a84ff; }
  `;
  configureClasses(await compileUtilities(new Set(['bg-surface/80', 'text-foreground', 'text-primary', 'dark:text-white']), css));
  assert.deepEqual(classStyle('bg-surface/80 text-foreground', { theme: 'light' }), { background: '#ffffffcc', color: '#1c1c1eff' });
  assert.deepEqual(classStyle('bg-surface/80 text-foreground'), { background: '#ffffffcc', color: '#1c1c1eff' });
  assert.deepEqual(classStyle('bg-surface/80 text-foreground', { theme: 'dark' }), { background: '#1f1f22cc', color: '#f2f2f7ff' });
  assert.equal(classStyle('text-primary', { theme: 'light' }).color, '#007affff');
  assert.equal(classStyle('text-primary', { theme: 'dark' }).color, '#0a84ffff');
  assert.equal(classStyle('dark:text-white text-primary', { theme: 'dark' }).color, '#ffffffff');
});

test('palette rules obey source order and diagnose invalid variables in either appearance', async () => {
  configureClasses(await compileUtilities(new Set(['bg-surface']), `
    @theme inline { --color-surface: var(--fill); }
    [data-theme="dark"] { --fill: black; }
    :root { --fill: white; }
    [data-theme="light"] { --fill: red; }
  `));
  assert.equal(classStyle('bg-surface', { theme: 'dark' }).background, '#ffffffff');
  assert.equal(classStyle('bg-surface', { theme: 'light' }).background, '#ff0000ff');
  for (const value of ['var(--missing)', 'var(--fill)']) {
    configureClasses(await compileUtilities(new Set(['bg-surface']), `@theme inline { --color-surface: var(--fill); } :root { --fill: white; } [data-theme="dark"] { --fill: ${value}; }`));
    assert.throws(() => classStyle('bg-surface', { theme: 'light' }), /UI_CLASS.*(?:unresolved|cyclic)/);
  }
  for (const css of ['[data-theme="dark"] { color-scheme: light; }', '[data-theme="dark"] { --fill: black !important; }'])
    await assert.rejects(compileUtilities(new Set(['p-4']), css), /UI_CSS/);
  await assert.rejects(compileUtilities(new Set(['p-4']), '@theme { @keyframes spin {} }'), /UI_ANIMATION/);
});

test('relative line height follows inline font size and inheritance', async () => {
  configureClasses(await compileUtilities(new Set(['text-sm', 'leading-normal', 'leading-5', 'text-[20px]'])));
  const parent = resolveStyle('text-sm', { fontSize: 21 });
  assert.equal(parent.style.lineHeight, 30);
  const child = resolveStyle('text-[20px]', undefined, {}, parent.typography);
  assert.ok(Math.abs(child.style.lineHeight - 20 * 20 / 14) < 1e-9);
  assert.equal(resolveStyle('leading-5', { fontSize: 21 }).style.lineHeight, 20);
  assert.equal(resolveStyle('leading-normal', { fontSize: 20 }).style.lineHeight, 30);
  assert.equal(resolveStyle('text-sm', { lineHeight: 40 }).style.lineHeight, 40);
});

test('relative typography crosses component and virtual list boundaries before encoding', async () => {
  configureClasses(await compileUtilities(new Set(['text-sm', 'text-[21px]'])));
  const Label = ({ children }) => h('text', { className: 'text-[21px]' }, children);
  const records = describe(() => h('column', { className: 'text-sm' },
    h(Label, {}, 'child'), h('list', { count: 1, row: () => h(Label, {}, 'row') })), {});
  const text = new Map();
  for (let i = 0; i < records.length;) {
    const count = records[i + 5], style = new Map();
    for (let j = 0; j < count; j++) style.set(records[i + 6 + j * 2], records[i + 7 + j * 2]);
    if (records[i] === 2) text.set(records[i + 4], style);
    i += 6 + 2 * count;
  }
  assert.equal(text.get('child').get(37), 30);
  assert.equal(text.get('row').get(37), 30);
});

test('padding utilities retain shorthand, axis, edge and variant cascade semantics', async () => {
  configureClasses(await compileUtilities(new Set(['p-2', 'px-6', 'py-3', 'pl-1', 'md:p-4', 'dark:px-3', '[padding:1px_2px_3px_4px]'])));
  const names = 'pl-1 px-6 p-2 py-3 md:p-4 dark:px-3';
  assert.deepEqual(classStyle(names, { theme: 'light', viewport: { width: 600 } }), { paddingTop: 12, paddingRight: 24, paddingBottom: 12, paddingLeft: 4 });
  assert.deepEqual(classStyle(names, { theme: 'light', viewport: { width: 800 } }), { paddingTop: 16, paddingRight: 16, paddingBottom: 16, paddingLeft: 16 });
  assert.deepEqual(classStyle(names, { theme: 'dark', viewport: { width: 800 } }), { paddingTop: 16, paddingRight: 12, paddingBottom: 16, paddingLeft: 12 });
  assert.deepEqual(classStyle('[padding:1px_2px_3px_4px]'), { paddingTop: 1, paddingRight: 2, paddingBottom: 3, paddingLeft: 4 });
  assert.deepEqual(resolveStyle('px-6 py-3', { paddingLeft: 7, padding: 5 }).style, { paddingTop: 5, paddingRight: 5, paddingBottom: 5, paddingLeft: 7, padding: 5 });
  assert.deepEqual(resolveStyle('px-6 py-3', { paddingTop: 8 }).style, { paddingTop: 8, paddingRight: 24, paddingBottom: 12, paddingLeft: 24 });
});

test('word-break utilities preserve native enum values and inline overrides', async () => {
  configureClasses(await compileUtilities(new Set(['break-all', 'break-normal', 'break-keep', 'wrap-anywhere', 'wrap-break-word', 'wrap-normal'])));
  assert.equal(classStyle('break-all').wordBreak, 'break-all');
  assert.deepEqual(classStyle('break-normal'), { wordBreak: 'normal', overflowWrap: 'normal' });
  assert.equal(classStyle('wrap-anywhere').overflowWrap, 'anywhere');
  assert.equal(classStyle('wrap-break-word').overflowWrap, 'break-word');
  assert.equal(classStyle('wrap-normal').overflowWrap, 'normal');
  assert.equal(classStyle('break-keep').wordBreak, 'keep-all');
  assert.equal(resolveStyle('break-all', {wordBreak:'normal'}).style.wordBreak, 'normal');
  assert.throws(() => describe(() => h('text', {style:{wordBreak:'invalid'}}, 'address')), /UI_STYLE_VALUE/);
});

test('solid border widths, colors and inline overrides retain Tailwind cascade', async () => {
  configureClasses(await compileUtilities(new Set(['border', 'border-0', 'border-4', 'border-t-2', 'border-x-8', 'border-blue-500/50', 'border-current', 'border-solid', 'border-dashed', 'border-none', '[border-width:1px_2px_3px_4px]'])));
  assert.deepEqual(classStyle('border'), {borderTopWidth:1,borderRightWidth:1,borderBottomWidth:1,borderLeftWidth:1});
  assert.deepEqual(classStyle('border-4 border-t-2'), classStyle('border-t-2 border-4'));
  assert.equal(classStyle('border-4 border-t-2').borderTopWidth,2);
  assert.deepEqual(classStyle('border-x-8'), {borderLeftWidth:8,borderRightWidth:8});
  assert.deepEqual(classStyle('[border-width:1px_2px_3px_4px]'), {borderTopWidth:1,borderRightWidth:2,borderBottomWidth:3,borderLeftWidth:4});
  assert.equal(classStyle('border-current').borderColor,'currentColor');
  assert.match(classStyle('border-blue-500/50').borderColor,/80$/);
  assert.equal(resolveStyle('border-4 border-t-2',{borderWidth:3,borderLeftWidth:7}).style.borderTopWidth,3);
  assert.equal(resolveStyle('border-4 border-t-2',{borderWidth:3,borderLeftWidth:7}).style.borderLeftWidth,7);
  for (const name of ['border-dashed','border-none']) assert.throws(()=>classStyle(name),/UI_CLASS/);
  assert.throws(()=>describe(()=>h('column',{style:{borderTopWidth:-1}})),/UI_STYLE_VALUE/);
});


test('line-through lowers to explicit text decoration and rejects unsupported line styles', async () => {
  configureClasses(await compileUtilities(new Set(['line-through', 'no-underline', 'underline'])));
  assert.deepEqual(classStyle('line-through'), {textDecorationLine:'line-through'});
  assert.deepEqual(classStyle('no-underline'), {textDecorationLine:'none'});
  assert.equal(resolveStyle('line-through', {textDecorationLine:'none'}).style.textDecorationLine, 'none');
  assert.throws(() => classStyle('underline'), /UI_CLASS/);
  assert.throws(() => describe(() => h('text', {style:{textDecorationLine:'invalid'}}, 'text')), /UI_STYLE_VALUE/);
});

test('explicit h-auto preserves intrinsic height with min/max limits', async () => {
  configureClasses(await compileUtilities(new Set(['h-auto','min-h-[42px]','max-h-[120px]'])));
  assert.deepEqual(classStyle('h-auto min-h-[42px] max-h-[120px]'), {height:'auto', minHeight:42, maxHeight:120});
  const buffer = describe(() => h('textarea', {className:'h-auto min-h-[42px] max-h-[120px]',value:'hello'}), {});
  const fields = new Map(Array.from({length:buffer[5]},(_,i)=>buffer.slice(6+i*2,8+i*2)));
  assert.equal(fields.get(82),1);
  assert.equal(fields.has(1),false);
  assert.equal(fields.has(75),false);
});

test('italic and not-italic lower to explicit native font styles', async () => {
  configureClasses(await compileUtilities(new Set(['italic','not-italic'])));
  assert.deepEqual(classStyle('italic'), {fontStyle:'italic'});
  assert.deepEqual(classStyle('not-italic'), {fontStyle:'normal'});
  assert.equal(resolveStyle('italic',{fontStyle:'normal'}).style.fontStyle,'normal');
  assert.throws(()=>describe(()=>h('text',{style:{fontStyle:'oblique'}},'invalid'),{}),/UI_STYLE/);
});

test('flex wrapping uses official Tailwind utilities and validates style values', async () => {
  configureClasses(await compileUtilities(new Set(['flex-wrap','flex-nowrap','flex-wrap-reverse'])));
  assert.deepEqual(classStyle('flex-wrap'), {flexWrap:'wrap'});
  assert.deepEqual(classStyle('flex-nowrap'), {flexWrap:'nowrap'});
  assert.deepEqual(classStyle('flex-wrap-reverse'), {flexWrap:'wrap-reverse'});
  assert.equal(resolveStyle('flex-wrap', {flexWrap:'nowrap'}).style.flexWrap, 'nowrap');
  assert.throws(()=>describe(()=>h('row',{style:{flexWrap:'invalid'}}),{}), /UI_STYLE/);
});


test('tracking resolves relative lengths on the owner and permits signed spacing', async () => {
  configureClasses(await compileUtilities(new Set(['text-[10px]','tracking-widest','tracking-tight','tracking-normal','tracking-[2px]','tracking-[0.2em]','tracking-[-1px]'])));
  assert.equal(classStyle('text-[10px] tracking-widest').letterSpacing,1);
  assert.equal(classStyle('text-[10px] tracking-tight').letterSpacing,-0.25);
  assert.equal(classStyle('tracking-normal').letterSpacing,0);
  assert.equal(classStyle('tracking-[2px]').letterSpacing,2);
  assert.equal(classStyle('tracking-[-1px]').letterSpacing,-1);
  assert.equal(resolveStyle('tracking-[0.2em]', {fontSize:20}).style.letterSpacing,4);
  assert.equal(resolveStyle('tracking-widest', {letterSpacing:0}).style.letterSpacing,0);
  assert.equal(resolveStyle('tracking-widest', undefined, {}, {fontSize:30}).style.letterSpacing,3);
  assert.throws(() => resolveStyle('tracking-widest', {letterSpacing:{em:2}}), /UI_STYLE_VALUE/);
  for(const value of [NaN,Infinity,1025,-1025,'1px'])
    assert.throws(() => describe(() => h('text',{style:{letterSpacing:value}},'Title'),{data:{},env:{}}), /UI_STYLE_VALUE/);
});


test('independent corners preserve Tailwind cascade and inline radius overrides', async () => {
  configureClasses(await compileUtilities(new Set(['rounded-3xl','rounded-br-md','rounded-bl-md','rounded-t-none','rounded-br-full'])));
  const expected={radius:24,...corners(24),radiusBottomRight:6};
  assert.deepEqual(classStyle('rounded-3xl rounded-br-md'),expected);
  assert.deepEqual(classStyle('rounded-br-md rounded-3xl'),expected);
  assert.deepEqual(resolveStyle('rounded-3xl rounded-br-md',{radius:8}).style,{radius:8,...corners(8)});
  assert.deepEqual(resolveStyle('rounded-3xl',{radius:8,radiusBottomLeft:0}).style,{radius:8,...corners(8),radiusBottomLeft:0});
  assert.deepEqual(classStyle('rounded-3xl rounded-t-none'),{radius:24,...corners(24),radiusTopLeft:0,radiusTopRight:0});
  assert.equal(classStyle('rounded-br-full').radiusBottomRight,16384);
  for(const value of [-1,16385,NaN,'6px']) assert.throws(()=>describe(()=>h('column',{style:{radiusBottomRight:value}}),{data:{},env:{}}),/UI_STYLE_VALUE/);
});


test('backdrop variables compose through CSS order, variants and disabling', async () => {
  const names = ['backdrop-blur-xl','backdrop-blur-none','backdrop-saturate-180','md:backdrop-filter-none','dark:backdrop-blur-sm','backdrop-brightness-125','backdrop-blur-[65px]','backdrop-saturate-[1700%]','[backdrop-filter:blur(20px)_saturate(180%)]','[backdrop-filter:saturate(180%)_blur(20px)]'];
  configureClasses(await compileUtilities(new Set(names)));
  const classes = 'backdrop-blur-xl backdrop-saturate-180 md:backdrop-filter-none dark:backdrop-blur-sm';
  const env = {theme:'light',viewport:{width:400}};
  assert.deepEqual(classStyle(classes,env), {backdropBlur:24,backdropSaturation:1.8});
  assert.deepEqual(classStyle(classes.split(' ').reverse().join(' '),env),classStyle(classes,env));
  assert.deepEqual(classStyle(classes,{...env,theme:'dark'}),{backdropBlur:8,backdropSaturation:1.8});
  assert.deepEqual(classStyle(classes,{...env,viewport:{width:900}}),{});
  assert.deepEqual(classStyle('backdrop-blur-none'),{});
  assert.deepEqual(classStyle('backdrop-blur-none backdrop-saturate-180'),{backdropBlur:0,backdropSaturation:1.8});
  assert.deepEqual(classStyle('[backdrop-filter:blur(20px)_saturate(180%)]'),{backdropBlur:20,backdropSaturation:1.8});
  for(const name of ['backdrop-brightness-125','backdrop-blur-[65px]','backdrop-saturate-[1700%]','[backdrop-filter:saturate(180%)_blur(20px)]']) assert.throws(()=>classStyle(name), /UI_CLASS/);
});


test('placeholder color remains separate from text across theme, viewport and inline overrides', async () => {
  configureClasses(await compileUtilities(new Set(['text-blue-500','placeholder:text-[#123456]/30','dark:placeholder:text-[#abcdef]','md:placeholder:text-red-500','placeholder:font-bold','placeholder:bg-red-500'])));
  const env={theme:'light',viewport:{width:400}};
  const names='text-blue-500 placeholder:text-[#123456]/30 dark:placeholder:text-[#abcdef] md:placeholder:text-red-500';
  const light=classStyle(names,env),dark=classStyle(names,{...env,theme:'dark'});
  assert.equal(light.placeholderColor,'#1234564d');
  assert.equal(dark.placeholderColor,'#abcdefff');
  assert.equal(light.color,dark.color);
  assert.deepEqual(light,classStyle(names.split(' ').reverse().join(' '),env));
  assert.notEqual(classStyle(names,{...env,viewport:{width:900}}).placeholderColor,light.placeholderColor);
  assert.equal(resolveStyle(names,{placeholderColor:'#987654'},env).style.placeholderColor,'#987654');
  for(const name of ['placeholder:font-bold','placeholder:bg-red-500']) assert.throws(()=>classStyle(name),/placeholder supports only color/);
});


test('editor focus paint preserves cascade and inline precedence without affecting base style', async () => {
  const names=['bg-white','focus:bg-red-500','md:bg-blue-500','dark:focus:bg-blue-500','focus:shadow-lg','focus:placeholder:text-[#123456]','focus:p-4','focus:text-lg'];
  configureClasses(await compileUtilities(new Set(names)));
  const classes='bg-white focus:bg-red-500 dark:focus:bg-blue-500 focus:shadow-lg focus:placeholder:text-[#123456]';
  const env={theme:'light',viewport:{width:400}};
  assert.deepEqual(classStyle(classes,env),{background:'#ffffffff'});
  const focus=focusStyle(classes,undefined,env);
  assert.equal(focus.placeholderColor,'#123456ff');assert.ok(focus.boxShadow.length);
  assert.notEqual(focus.background,focusStyle(classes,undefined,{...env,theme:'dark'}).background);
  assert.deepEqual(focus,focusStyle(classes.split(' ').reverse().join(' '),undefined,env));
  assert.equal(focusStyle('bg-white focus:bg-red-500',{background:'#abcdef'},env),undefined);
  assert.equal(focusStyle('focus:bg-red-500 md:bg-blue-500',undefined,{...env,viewport:{width:900}}).background,focus.background);
  for(const name of ['focus:p-4','focus:text-lg']) assert.throws(()=>classStyle(name),/focus supports paint properties only/);
  const encoded=describe(()=>h('input',{className:classes}),{env});
  const index=encoded.indexOf(98);assert.ok(index>0);assert.ok(JSON.parse(encoded[index+1]).includes(97));
  assert.throws(()=>describe(()=>h('button',{className:'focus:bg-red-500'}),{env}),/focus styles require an editor/);
});


test('outer rings compose before shadows and fixed shadows replace the composition', async () => {
  configureClasses(await compileUtilities(new Set(['ring-2','ring-0','ring-blue-500/30','shadow-lg','shadow-none','focus:ring-2','focus:ring-blue-500/30','ring-offset-2','ring-inset','[box-shadow:none]'])));
  const style=classStyle('ring-2 ring-blue-500/30 shadow-lg');
  assert.equal(style.boxShadow.length,3);assert.equal(style.boxShadow[0].spread,2);assert.match(style.boxShadow[0].color,/4d$/);
  assert.equal(classStyle('ring-2 shadow-none').boxShadow[0].spread,2);
  assert.deepEqual(classStyle('ring-0').boxShadow,[]);
  assert.deepEqual(classStyle('ring-2 [box-shadow:none]').boxShadow,[]);
  assert.deepEqual(classStyle('focus:ring-2 focus:ring-blue-500/30'),{});
  assert.equal(focusStyle('focus:ring-2 focus:ring-blue-500/30').boxShadow[0].spread,2);
  for(const name of ['ring-offset-2','ring-inset']) assert.throws(()=>classStyle(name),/unsupported shadow property/);
});
