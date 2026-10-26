import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compileUtilities } from '../compiler/tailwind.mjs';
import { configureClasses, resolveClasses } from '../src/style/classes.js';
import { describe, h } from '../src/internal.js';
import { rendered, decode } from './support/wire.mjs';

// Tailwind rules lower to CSS-named style; rendering then lowers that to the native box.
const classStyle = (name, env) => resolveClasses(name, env)?.base ?? {};
const use = async (names, theme) => configureClasses(await compileUtilities(new Set(names), theme));
const env = (theme = 'light', width = 400) => ({ theme, viewport: { width, height: 800 } });
const node = (tag, props, view = {}) => rendered(() => h(tag, props, ...(tag === 'text' ? ['x'] : [])), view).props;

test('flex direction follows utility cascade, responsive rules and explicit style', async () => {
  await use(['flex', 'flex-col', 'md:flex-row', 'flex-row-reverse', 'flex-col-reverse']);
  assert.deepEqual(classStyle('flex flex-col md:flex-row', env('light', 360)), { display: 'flex', flexDirection: 'column' });
  assert.deepEqual(classStyle('md:flex-row flex-col flex', env('light', 900)), { display: 'flex', flexDirection: 'row' });
  assert.equal(node('view', { className: 'flex-col', style: { flexDirection: 'row-reverse' } }).flexDirection, 'row-reverse');
  assert.equal(classStyle('flex-col-reverse').flexDirection, 'column-reverse');
  const view = rendered(() => h('view', { className: 'flex flex-col' }, h('text', {}, 'Hello')));
  assert.deepEqual([view.tag, view.props.display, view.props.flexDirection], ['column', 'flex', 'column']);
  assert.throws(() => describe(() => h('grid', { className: 'flex-col', style: { gridTemplateColumns: 2 } }), {}), /flexDirection is not supported on <grid>/);
});

test('display utilities, fixed positioning and dynamic viewport units lower to defined native semantics', async () => {
  await use(['hidden', 'block', 'inline-flex', 'inline-block', 'md:hidden', 'contents', 'table', 'fixed', 'sticky', 'h-dvh', 'min-h-svh', 'max-h-lvh', 'w-dvw', 'h-[50dvh]']);
  assert.deepEqual(classStyle('hidden'), { display: 'none' });
  assert.deepEqual(classStyle('block md:hidden', env('light', 900)), { display: 'none' });
  for (const name of ['block', 'inline-flex', 'inline-block']) assert.equal(classStyle(name).display, name);
  assert.equal(node('text', { className: 'hidden' }).display, 'none');
  assert.equal(node('image', { src: 'a.png', className: 'hidden' }).display, 'none');
  for (const name of ['contents', 'table', 'sticky']) assert.throws(() => classStyle(name), /UI_CLASS/);
  assert.equal(classStyle('fixed').position, 'fixed');
  assert.deepEqual([classStyle('h-dvh').height, classStyle('min-h-svh').minHeight, classStyle('max-h-lvh').maxHeight, classStyle('w-dvw').width, classStyle('h-[50dvh]').height],
    ['100vh', '100vh', '100vh', '100vw', '50vh']);
  assert.equal(node('column', { className: 'h-dvh' }, { env: { viewport: { width: 400, height: 700 } } }).height, 700);
});

test('static transforms compose per-axis variables, origins and rotations; skew and 3D fail', async () => {
  await use(['translate-x-2', '-translate-y-1/2', 'translate-[10px_20%]', 'rotate-45', '-rotate-90', 'scale-95', '-scale-x-100', 'scale-y-50',
    'origin-top-left', 'origin-[25%_75%]', 'skew-x-6', 'rotate-x-45', 'transform', 'active:scale-95', 'md:rotate-180']);
  assert.deepEqual(classStyle('translate-x-2 -translate-y-1/2').translate, [8, '-50%']);
  assert.deepEqual(classStyle('-translate-y-1/2 translate-x-2'), classStyle('translate-x-2 -translate-y-1/2'));
  assert.deepEqual(classStyle('translate-[10px_20%]').translate, [10, '20%']);
  assert.deepEqual(classStyle('-scale-x-100').scale, [-1, 1]);
  assert.deepEqual(classStyle('scale-95 scale-y-50').scale, [.95, .5]);
  assert.deepEqual([classStyle('-rotate-90').rotate, classStyle('rotate-45 md:rotate-180', env('light', 900)).rotate], [-90, 180]);
  assert.deepEqual([classStyle('origin-top-left').transformOrigin, classStyle('origin-[25%_75%]').transformOrigin], [[0, 0], [.25, .75]]);
  assert.deepEqual(classStyle('transform'), {});
  for (const name of ['skew-x-6', 'rotate-x-45']) assert.throws(() => classStyle(name), /only translate, rotate and scale/);
  assert.deepEqual(JSON.parse(node('column', { className: 'translate-x-2 rotate-45 -scale-x-100 origin-top-left' }).transform),
    { translate: [8, 0], rotate: 45, scale: [-1, 1], origin: [0, 0] });
  assert.throws(() => describe(() => h('button', { className: 'active:scale-95' }, 'Go'), {}), /paint properties only, not scale/);
});

test('numeric variants compose across classes and white-space keeps CSS keywords', async () => {
  await use(['tabular-nums', 'slashed-zero', 'normal-nums', 'oldstyle-nums', '[font-variant-numeric:tabular-nums_ordinal]',
    'whitespace-pre-line', 'whitespace-pre-wrap', 'whitespace-pre', 'whitespace-nowrap', 'whitespace-break-spaces', 'break-all']);
  assert.equal(classStyle('tabular-nums slashed-zero').fontVariantNumeric, 'slashed-zero tabular-nums');
  assert.equal(classStyle('slashed-zero tabular-nums').fontVariantNumeric, 'slashed-zero tabular-nums');
  assert.equal(classStyle('tabular-nums normal-nums').fontVariantNumeric, 'normal');
  assert.equal(node('text', { className: '[font-variant-numeric:tabular-nums_ordinal]' }).fontVariantNumeric, 'ordinal tabular-nums');
  for (const name of ['pre-line', 'pre-wrap', 'pre', 'nowrap']) assert.equal(classStyle(`whitespace-${name}`).whiteSpace, name);
  assert.throws(() => classStyle('whitespace-break-spaces'), /UI_CLASS/);
  assert.equal(classStyle('break-all').wordBreak, 'break-all');
  assert.throws(() => describe(() => h('text', { style: { fontVariantNumeric: 'tabular-nums proportional-nums' } }, '1'), {}), /UI_STYLE_VALUE/);
  assert.throws(() => describe(() => h('text', { style: { fontVariantNumeric: 'small-caps' } }, '1'), {}), /UI_STYLE_VALUE/);
});

test('aspect ratios, ch widths, current-color backgrounds, overscroll and select-none lower or fail by name', async () => {
  await use(['aspect-square', 'aspect-video', 'aspect-[4/3]', 'aspect-auto', 'max-w-[60ch]', 'w-[10ch]', 'bg-current', 'overscroll-contain',
    'overscroll-y-none', 'select-none', 'select-text', 'select-all']);
  assert.deepEqual([classStyle('aspect-square').aspectRatio, classStyle('aspect-video').aspectRatio, classStyle('aspect-[4/3]').aspectRatio], [1, 16 / 9, 4 / 3]);
  assert.equal(node('column', { className: 'aspect-video', style: { aspectRatio: 'auto' } }).aspectRatio, undefined);
  assert.equal(node('text', { className: 'max-w-[60ch]' }).maxWidthCh, 60);
  assert.throws(() => describe(() => h('column', { className: 'w-[10ch]' }), {}), /UI_STYLE_VALUE: width/);
  assert.equal(node('column', { className: 'bg-current' }).background, 'currentColor');
  assert.equal(node('scroll', { className: 'overscroll-contain' }).overscroll, 'contain');
  assert.equal(node('scroll', { className: 'overscroll-y-none' }).overscroll, 'none');
  assert.throws(() => describe(() => h('column', { className: 'overscroll-contain' }), {}), /overscrollBehavior is not supported on <column>/);
  assert.deepEqual(classStyle('select-none'), {});
  for (const name of ['select-text', 'select-all']) assert.throws(() => classStyle(name), /selectable text is not supported yet/);
});

test('CSS grid utilities lower to tracks, placements, alignment and order; named lines and areas fail', async () => {
  await use(['grid', 'grid-cols-3', 'grid-rows-[auto_1fr]', 'grid-cols-[repeat(auto-fit,minmax(7.5rem,1fr))]', 'col-span-2', 'col-start-2',
    'col-end-4', 'row-span-full', 'grid-flow-col-dense', 'auto-rows-fr', 'place-items-center', 'content-between', 'place-self-end',
    'order-2', 'order-first', 'grid-cols-[[a]_1fr]', 'grid-cols-[subgrid]', '[grid-template-areas:"a_b"]']);
  assert.deepEqual(classStyle('grid grid-cols-3'), { display: 'grid', gridTemplateColumns: 'repeat(3, minmax(0, 1fr))' });
  assert.equal(classStyle('grid-cols-[repeat(auto-fit,minmax(7.5rem,1fr))]').gridTemplateColumns, 'repeat(auto-fit,minmax(120px,1fr))');
  const item = node('column', { className: 'col-span-2 col-end-4 row-span-full place-self-end order-first' });
  assert.deepEqual([item.gridColumn, item.gridRow, item.alignSelf, item.justifySelf, item.order], ['span 2 / 4', '1 / -1', 'end', 'end', -9999]);
  assert.equal(node('column', { className: 'col-start-2' }).gridColumn, '2');
  const box = node('view', { className: 'grid grid-flow-col-dense auto-rows-fr place-items-center content-between' });
  assert.deepEqual([box.display, box.gridAutoFlow, box.gridAutoRows, box.align, box.justifyItems, box.alignContent],
    ['grid', 'column-dense', 'minmax(0, 1fr)', 'center', 'center', 'between']);
  for (const name of ['grid-cols-[[a]_1fr]', 'grid-cols-[subgrid]']) assert.throws(() => classStyle(name), /named lines, subgrid and calc/);
  assert.throws(() => classStyle('[grid-template-areas:"a_b"]'), /UI_CLASS/);
});

test('flow-relative utilities stay logical on the wire and rtl:/ltr: follow the inherited direction', async () => {
  await use(['ps-4', 'pe-2', 'ms-auto', 'me-1', 'start-0', 'end-2', 'border-s-2', 'rounded-s-lg', 'rounded-ee-md', 'text-start',
    'rtl:-scale-x-100', 'ltr:ml-2', 'rtl:mr-2', 'pl-1', 'px-3']);
  const props = node('column', { className: 'ps-4 pe-2 ms-auto start-0 border-s-2 rounded-s-lg rounded-ee-md' });
  assert.deepEqual([props.paddingInlineStart, props.paddingInlineEnd, props.marginInlineStart, props.insetInlineStart, props.borderInlineStartWidth],
    [16, 8, 'auto', 0, 2]);
  assert.deepEqual([props.radiusStartStart, props.radiusEndStart, props.radiusEndEnd], [8, 8, 6]);
  assert.deepEqual(classStyle('rtl:-scale-x-100', { direction: 'ltr' }), {});
  assert.deepEqual(classStyle('rtl:-scale-x-100', { direction: 'rtl' }).scale, [-1, 1]);
  const tree = rendered(() => h('column', { dir: 'rtl' }, h('vector', { className: 'rtl:-scale-x-100 ltr:ml-2', paths: [{ d: 'M0 0h1' }] }),
    h('column', { dir: 'ltr' }, h('column', { className: 'rtl:mr-2 ltr:ml-2' }))));
  assert.equal(tree.props.direction, 'rtl');
  assert.deepEqual(JSON.parse(tree.children[0].props.transform).scale, [-1, 1]);
  assert.equal(tree.children[0].props.marginLeft, undefined);
  assert.deepEqual([tree.children[1].children[0].props.marginLeft, tree.children[1].children[0].props.marginRight], [8, undefined]);
  assert.equal(rendered(() => h('column', { className: 'rtl:mr-2' }), { env: { direction: 'rtl' } }).props.marginRight, 8);
  assert.throws(() => describe(() => h('column', { dir: 'auto' }), {}), /UI_PROP: dir/);
});

test('horizontal overflow lowers only onto scroll containers', async () => {
  await use(['overflow-x-auto', 'md:overflow-x-auto', 'overflow-hidden']);
  assert.deepEqual(classStyle('overflow-x-auto'), { overflowX: 'auto' });
  assert.deepEqual(classStyle('md:overflow-x-auto', env('light', 320)), {});
  assert.deepEqual(classStyle('md:overflow-x-auto', env('light', 900)), { overflowX: 'auto' });
  assert.equal(node('scroll', { className: 'overflow-x-auto' }).scrollAxis, 'horizontal');
  assert.throws(() => describe(() => h('row', { className: 'overflow-x-auto' }), {}), /overflowX is not supported on <row>/);
  assert.equal(node('row', { className: 'overflow-hidden' }).overflow, 'hidden');
});

test('alignment keeps CSS values until rendering', async () => {
  await use(['items-baseline', 'self-baseline', 'justify-between', 'justify-around', 'justify-evenly', 'items-start']);
  assert.deepEqual(classStyle('items-baseline self-baseline'), { alignItems: 'baseline', alignSelf: 'baseline' });
  assert.deepEqual(classStyle('justify-around'), { justifyContent: 'space-around' });
  const row = node('row', { className: 'items-start justify-evenly' });
  assert.deepEqual([row.align, row.justify], ['start', 'evenly']);
  assert.doesNotThrow(() => describe(() => h('row', { className: 'items-baseline' }, h('text', { className: 'self-baseline' }, 'Price')), {}));
});

test('cached class cascades isolate mutations, typography and environments', async () => {
  await use(['p-4', 'leading-relaxed', 'tracking-wider', 'shadow-sm', 'text-red-500', 'dark:text-white', 'md:p-8']);
  const classes = 'p-4 leading-relaxed tracking-wider shadow-sm text-red-500 dark:text-white md:p-8';
  const first = resolveClasses(classes, env());
  first.base.boxShadow[0].color = '#123456'; first.base.paddingTop = 900;
  const second = node('text', { className: classes, style: { fontSize: 20 } }, { env: env() });
  assert.equal(second.paddingTop, 16); assert.equal(second.lineHeight, 32.5); assert.equal(second.letterSpacing, 1);
  assert.notEqual(JSON.parse(second.boxShadow)[0].color, '#123456');
  const dark = classStyle(classes, env('dark', 900));
  assert.equal(dark.paddingTop, 32); assert.notEqual(dark.color, classStyle(classes, env()).color);
  configureClasses([]); assert.throws(() => classStyle(classes), /was not compiled/);
});

test('flex growth, basis and shorthands retain independent cascade values', async () => {
  await use(['grow', 'basis-0', 'basis-auto', 'basis-1/2', 'basis-12', 'flex-1', 'flex-auto', 'flex-none', 'flex-initial', 'md:basis-12']);
  assert.deepEqual(classStyle('grow'), { flexGrow: 1 });
  assert.deepEqual(classStyle('grow basis-0'), { flexGrow: 1, flexBasis: 0 });
  assert.deepEqual(classStyle('flex-1'), { flexGrow: 1, flexShrink: 1, flexBasis: '0%' });
  assert.deepEqual(classStyle('flex-auto'), { flexGrow: 1, flexShrink: 1, flexBasis: 'auto' });
  assert.deepEqual(classStyle('flex-none'), { flexGrow: 0, flexShrink: 0, flexBasis: 'auto' });
  assert.deepEqual(classStyle('flex-initial'), { flexGrow: 0, flexShrink: 1, flexBasis: 'auto' });
  assert.equal(classStyle('basis-1/2').flexBasis, '50%');
  assert.equal(classStyle('basis-1/2 md:basis-12', env('light', 900)).flexBasis, 48);
  assert.deepEqual(classStyle('flex-1 basis-auto'), classStyle('basis-auto flex-1'));
  assert.equal(node('column', { className: 'basis-12', style: { flexBasis: '25%' } }).flexBasis, '25%');
});

test('percentages, fractions, viewport units and sizing keywords preserve relative layout', async () => {
  await use(['w-full', 'w-1/2', 'h-1/3', 'max-w-[72%]', 'min-h-full', 'md:w-1/2', 'gap-[10%]', 'w-screen', 'h-screen', 'w-fit', 'w-auto']);
  const { height, ...rest } = classStyle('w-full h-1/3 max-w-[72%] min-h-full');
  assert.deepEqual(rest, { width: '100%', maxWidth: '72%', minHeight: '100%' });
  assert.ok(Math.abs(parseFloat(height) - 100 / 3) < 1e-5);
  assert.equal(classStyle('w-full md:w-1/2', env('light', 800)).width, '50%');
  assert.deepEqual(classStyle('w-screen h-screen'), { width: '100vw', height: '100vh' });
  const screen = node('column', { className: 'w-screen h-screen' }, { env: { viewport: { width: 390, height: 844 } } });
  assert.deepEqual([screen.width, screen.height], [390, 844]);
  assert.equal(node('column', { className: 'w-fit' }).widthKeyword, 'fit-content');
  assert.deepEqual(classStyle('w-auto'), { width: 'auto' });
  assert.throws(() => classStyle('gap-[10%]'), /UI_CLASS/);
});

test('margins, positioning, layering and line clamps are native utilities', async () => {
  await use(['m-2', 'mx-auto', '-mt-1', 'relative', 'absolute', 'inset-0', 'inset-x-4', 'top-1/2', 'z-10', 'truncate', 'line-clamp-3', 'grid-cols-3', 'object-cover']);
  assert.deepEqual(classStyle('m-2 mx-auto -mt-1'), { marginTop: -4, marginRight: 'auto', marginBottom: 8, marginLeft: 'auto' });
  assert.deepEqual(classStyle('absolute inset-0 inset-x-4 z-10'), { position: 'absolute', top: 0, right: 16, bottom: 0, left: 16, zIndex: 10 });
  assert.equal(classStyle('top-1/2').top, '50%');
  assert.deepEqual(classStyle('truncate'), { overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' });
  assert.deepEqual(classStyle('line-clamp-3'), { overflow: 'hidden', lineClamp: 3 });
  assert.equal(node('text', { className: 'line-clamp-3' }).textLines, 3);
  assert.equal(node('grid', { className: 'grid-cols-3' }).gridTemplateColumns, 'repeat(3, minmax(0, 1fr))');
  assert.equal(node('image', { src: 'a.png', className: 'object-cover' }).imageFit, 'cover');
  assert.throws(() => describe(() => h('column', { className: 'line-clamp-3' }), {}), /lineClamp is not supported on <column>/);
});

test('space and divide utilities target non-last children', async () => {
  await use(['space-y-2', 'space-x-3', 'divide-y', 'divide-gray-200', 'gap-x-2', 'gap-y-4']);
  assert.deepEqual(resolveClasses('space-y-2').children, { marginTop: 0, marginBottom: 8 });
  assert.deepEqual(resolveClasses('space-x-3').children, { marginInlineStart: 0, marginInlineEnd: 12 });
  const list = rendered(() => h('column', { className: 'space-y-2 divide-y divide-gray-200' }, h('text', {}, 'a'), h('text', {}, 'b')));
  assert.deepEqual(list.children.map(child => [child.props.marginBottom, child.props.borderBottomWidth]), [[8, 1], [undefined, undefined]]);
  assert.match(list.children[0].props.borderColor, /^#e5e7eb/);
  assert.deepEqual(classStyle('gap-x-2 gap-y-4'), { columnGap: 8, rowGap: 16 });
});

test('state variants lower to state styles and disabled replaces the cascade', async () => {
  await use(['bg-white', 'active:bg-gray-100', 'hover:bg-gray-50', 'disabled:opacity-50', 'focus:bg-red-500']);
  const states = resolveClasses('bg-white active:bg-gray-100 hover:bg-gray-50 disabled:opacity-50', { pointer: 'fine' });
  assert.deepEqual(Object.keys(states.active), ['backgroundColor']);
  assert.deepEqual(Object.keys(states.hover), ['backgroundColor']);
  assert.equal(states.disabled.opacity, 0.5);
  assert.equal(resolveClasses('hover:bg-gray-50', { pointer: 'coarse' }).hover, undefined);
  const button = rendered(() => h('button', { className: 'bg-white active:bg-gray-100 disabled:opacity-50', disabled: true }, 'Save')).props;
  assert.deepEqual([button.opacity, button.pressedStyle, button.enabled], [0.5, undefined, 0]);
  assert.ok(rendered(() => h('input', { className: 'focus:bg-red-500' })).props.focusStyle.background);
});

test('text utilities: transform, decoration, nowrap, alignment and families', async () => {
  const aligns = ['left', 'center', 'right', 'start', 'end', 'justify'];
  await use(['uppercase', 'capitalize', 'underline', 'line-through', 'no-underline', 'whitespace-nowrap', 'font-mono', 'font-sans', 'font-serif',
    '[font-family:monospace]', '[font-family:Custom,sans-serif]', 'italic', 'not-italic', ...aligns.map(value => `text-${value}`)]);
  for (const value of aligns) assert.deepEqual(classStyle(`text-${value}`), { textAlign: value });
  assert.deepEqual(classStyle('uppercase'), { textTransform: 'uppercase' });
  assert.deepEqual(classStyle('underline'), { textDecorationLine: 'underline' });
  assert.deepEqual(classStyle('no-underline'), { textDecorationLine: 'none' });
  assert.deepEqual(classStyle('line-through'), { textDecorationLine: 'line-through' });
  assert.deepEqual(classStyle('whitespace-nowrap'), { whiteSpace: 'nowrap' });
  assert.deepEqual(classStyle('font-mono'), { fontFamily: 'monospace' });
  assert.deepEqual(classStyle('font-sans'), { fontFamily: 'sans-serif' });
  assert.deepEqual(classStyle('[font-family:monospace]'), { fontFamily: 'monospace' });
  assert.equal(node('text', { className: 'font-mono' }).fontFamily, 'mono');
  assert.deepEqual([classStyle('italic'), classStyle('not-italic')], [{ fontStyle: 'italic' }, { fontStyle: 'normal' }]);
  for (const name of ['font-serif', '[font-family:Custom,sans-serif]']) assert.throws(() => classStyle(name), /UI_CLASS/);
});

test('outer shadows preserve layers, color overrides, variants and CSS order', async () => {
  const names = ['shadow-sm', 'shadow-lg', 'shadow-none', 'shadow-red-500/40', 'dark:shadow-blue-500', 'md:shadow-none'];
  const rules = await compileUtilities(new Set(names));
  assert.deepEqual(rules.filter(rule => rule.error), []);
  configureClasses(rules);
  assert.deepEqual(classStyle('shadow-lg').boxShadow, [
    { offsetX: 0, offsetY: 10, blur: 15, spread: -3, color: '#0000001a' },
    { offsetX: 0, offsetY: 4, blur: 6, spread: -4, color: '#0000001a' },
  ]);
  const classes = 'shadow-sm shadow-red-500/40 dark:shadow-blue-500 md:shadow-none';
  const style = classStyle(classes, env());
  assert.deepEqual(style, classStyle(classes.split(' ').reverse().join(' '), env()));
  assert.equal(style.boxShadow.length, 2);
  assert.match(style.boxShadow[0].color, /66$/);
  assert.equal(style.boxShadow[0].color, style.boxShadow[1].color);
  assert.match(classStyle(classes, env('dark')).boxShadow[0].color, /ff$/);
  assert.equal(classStyle(classes, env('light', 800)).boxShadow[0].color, '#00000000');
  assert.deepEqual(classStyle('shadow-red-500/40'), {});
  assert.equal(node('column', { className: classes, style: { boxShadow: 'none' } }, { env: env() }).boxShadow, 'none');
});

test('arbitrary outer shadows retain currentColor and reject unsupported or excessive geometry', async () => {
  await use(['shadow-[0_2px_4px]', 'shadow-[2px_4px_0_3px_#ff0000]', 'shadow-[0_0_257px_black]', 'inset-shadow-sm', 'ring-offset-2', '[box-shadow:var(--tw-shadow)]', '[box-shadow:1_2_red]']);
  assert.deepEqual(classStyle('shadow-[0_2px_4px]').boxShadow, [{ offsetX: 0, offsetY: 2, blur: 4, spread: 0, color: 'currentColor' }]);
  assert.deepEqual(classStyle('shadow-[2px_4px_0_3px_#ff0000]').boxShadow, [{ offsetX: 2, offsetY: 4, blur: 0, spread: 3, color: '#ff0000ff' }]);
  for (const name of ['shadow-[0_0_257px_black]', 'inset-shadow-sm', 'ring-offset-2', '[box-shadow:var(--tw-shadow)]', '[box-shadow:1_2_red]'])
    assert.throws(() => classStyle(name), /UI_CLASS/);
});

test('shadow tokens and arbitrary variables expand to every layer they hold in each appearance', async () => {
  const theme = `@theme inline { --shadow-card: var(--card-shadow); }
    :root { --card-shadow: 0 1px 2px #0000001a, 0 4px 8px hsl(220 13% 20% / .1); --elev: 0 1px 2px #0002, 0 4px 8px #0003; --nine: ${Array(9).fill('0 1px 2px #000').join(', ')}; }
    [data-theme='dark'] { --card-shadow: 0 0 0 1px #ffffff1a; }`;
  await use(['shadow-card', 'shadow-[var(--elev)]', '[box-shadow:var(--elev)]', 'shadow-[var(--nine)]', 'shadow-red-500'], theme);
  assert.deepEqual(classStyle('shadow-card', env()).boxShadow.map(layer => layer.offsetY), [1, 4]);
  assert.deepEqual(classStyle('shadow-card', env('dark')).boxShadow, [{ offsetX: 0, offsetY: 0, blur: 0, spread: 1, color: '#ffffff1a' }]);
  assert.deepEqual(classStyle('shadow-[var(--elev)]').boxShadow.map(layer => layer.color), ['#00000022', '#00000033']);
  assert.deepEqual(classStyle('[box-shadow:var(--elev)]').boxShadow.length, 2);
  // A layer Tailwind did not wrap in --tw-shadow-color keeps its own color, as in CSS.
  assert.equal(classStyle('shadow-[var(--elev)] shadow-red-500').boxShadow[0].color, '#00000022');
  assert.throws(() => classStyle('shadow-[var(--nine)]'), /too many box shadows/);
});

test('outer rings compose before shadows and fixed shadows replace the composition', async () => {
  await use(['ring-2', 'ring-0', 'ring-blue-500/30', 'shadow-lg', 'shadow-none', 'focus:ring-2', 'focus:ring-blue-500/30', 'ring-offset-2', 'ring-inset', '[box-shadow:none]']);
  const style = classStyle('ring-2 ring-blue-500/30 shadow-lg');
  assert.equal(style.boxShadow.length, 3); assert.equal(style.boxShadow[0].spread, 2); assert.match(style.boxShadow[0].color, /4d$/);
  assert.equal(classStyle('ring-2 shadow-none').boxShadow[0].spread, 2);
  assert.deepEqual(classStyle('ring-0').boxShadow, []);
  assert.deepEqual(classStyle('ring-2 [box-shadow:none]').boxShadow, []);
  assert.deepEqual(classStyle('focus:ring-2 focus:ring-blue-500/30'), {});
  assert.equal(resolveClasses('focus:ring-2 focus:ring-blue-500/30').focus.boxShadow[0].spread, 2);
  for (const name of ['ring-offset-2', 'ring-inset']) assert.throws(() => classStyle(name), /unsupported shadow property/);
});

test('gradient utilities compose across classes using Tailwind cascade and theme', async () => {
  const names = ['bg-linear-to-br', 'bg-linear-to-r/srgb', 'from-red-500', 'via-green-500', 'to-blue-500', 'from-80%', 'via-10%', 'to-20%', 'dark:from-white', 'md:bg-none', 'bg-white'];
  const rules = await compileUtilities(new Set(names));
  assert.deepEqual(rules.filter(rule => rule.error), []);
  configureClasses(rules);
  const classes = 'bg-white bg-linear-to-br from-red-500 via-green-500 to-blue-500 from-80% via-10% to-20% dark:from-white md:bg-none';
  const style = classStyle(classes, env());
  assert.deepEqual(style, classStyle(classes.split(' ').reverse().join(' '), env()));
  assert.equal(style.backgroundColor, '#ffffffff');
  assert.equal(style.backgroundImage.direction, 'bottom-right');
  assert.equal(style.backgroundImage.colorSpace, 'oklab');
  assert.deepEqual(style.backgroundImage.stops.map(stop => stop.offset), [.8, .8, .8]);
  assert.equal(classStyle(classes, env('dark')).backgroundImage.stops[0].color, '#ffffffff');
  assert.equal(classStyle(classes, env('light', 800)).backgroundImage, 'none');
  assert.equal(classStyle('bg-linear-to-r/srgb from-red-500 to-blue-500').backgroundImage.colorSpace, 'srgb');
  assert.deepEqual(classStyle('from-red-500'), {});
});

test('gradient defaults, transparency, negative stops and unsupported spaces are explicit', async () => {
  await use(['bg-linear-45', 'from-red-500/50', 'to-blue-500', 'from-[-10%]', 'to-[120%]', 'bg-linear-to-r/hsl', 'via-[12px]']);
  const gradient = classStyle('bg-linear-45 from-red-500/50 from-[-10%] to-blue-500 to-[120%]').backgroundImage;
  assert.equal(gradient.angle, 45);
  assert.equal(gradient.stops[0].offset, -.1);
  assert.equal(gradient.stops[1].offset, 1.2);
  assert.match(gradient.stops[0].color, /80$/);
  assert.equal(classStyle('bg-linear-45 from-red-500/50').backgroundImage.stops[1].color, '#00000000');
  assert.equal(classStyle('bg-linear-45').backgroundImage, 'none');
  for (const name of ['bg-linear-to-r/hsl', 'via-[12px]']) assert.throws(() => classStyle(name), /UI_CLASS/);
});

test('radial, conic and complete CSS gradients lower from utilities, arbitrary values and theme tokens', async () => {
  const theme = `@theme { --gradient-hero: linear-gradient(135deg, #ffffff, #000000 80%); }
    :root { --glow: radial-gradient(circle at top, #ff000080, transparent 70%); }`;
  await use(['bg-radial', 'bg-radial-[at_25%_75%]', 'bg-conic-180', 'bg-conic', 'from-red-500', 'to-blue-500',
    'bg-[radial-gradient(circle_closest-side_at_left_top,#fff,#000_50%,#f00)]', 'bg-[image:var(--glow)]', 'bg-(image:--gradient-hero)',
    'bg-[conic-gradient(from_90deg,#fff_0deg,#000_0.5turn)]', 'bg-[repeating-linear-gradient(#fff,#000)]', 'bg-[radial-gradient(#fff_10px,#000)]',
    'bg-[linear-gradient(at_top,#fff,#000)]', 'bg-linear-45'], theme);
  const radial = classStyle('bg-radial-[at_25%_75%] from-red-500 to-blue-500').backgroundImage;
  assert.deepEqual([radial.kind, radial.at, radial.stops.length], ['radial', [.25, .75], 2]);
  assert.equal(classStyle('bg-radial from-red-500 to-blue-500').backgroundImage.colorSpace, 'oklab');
  assert.deepEqual(classStyle('bg-conic-180 from-red-500 to-blue-500').backgroundImage.from, 180);
  assert.deepEqual(classStyle('bg-[radial-gradient(circle_closest-side_at_left_top,#fff,#000_50%,#f00)]').backgroundImage, {
    kind: 'radial', shape: 'circle', size: 'closest-side', at: [0, 0], colorSpace: 'srgb',
    stops: [{ offset: 0, color: '#ffffffff' }, { offset: .5, color: '#000000ff' }, { offset: 1, color: '#ff0000ff' }],
  });
  const glow = classStyle('bg-[image:var(--glow)]').backgroundImage;
  assert.deepEqual([glow.shape, glow.at, glow.stops.map(stop => stop.offset)], ['circle', [.5, 0], [0, .7]]);
  const hero = classStyle('bg-(image:--gradient-hero)').backgroundImage;
  assert.deepEqual([hero.kind, hero.angle, hero.stops[1].offset], [undefined, 135, .8]);
  assert.deepEqual(classStyle('bg-[conic-gradient(from_90deg,#fff_0deg,#000_0.5turn)]').backgroundImage.stops.map(stop => stop.offset), [0, .5]);
  // Both set one background-image; Tailwind's order decides, never the class string.
  const mixed = 'bg-(image:--gradient-hero) bg-linear-45 from-red-500 to-blue-500';
  assert.deepEqual(classStyle(mixed), classStyle(mixed.split(' ').reverse().join(' ')));
  assert.equal(classStyle(mixed).backgroundImage.angle, 135);
  for (const [name, message] of [['bg-[repeating-linear-gradient(#fff,#000)]', /background-image/], ['bg-[radial-gradient(#fff_10px,#000)]', /percentages/],
    ['bg-[linear-gradient(at_top,#fff,#000)]', /linear gradients do not take at/]])
    assert.throws(() => classStyle(name), message);
});

test('dashed and dotted borders follow the border-style variable whatever the class order', async () => {
  await use(['border', 'border-2', 'border-t-4', 'border-dashed', 'border-dotted', 'border-solid', '[border-top-style:dashed]', 'md:border-dotted']);
  for (const classes of ['border border-dashed', 'border-dashed border', 'border-t-4 border-dashed'])
    assert.equal(classStyle(classes).borderStyle, 'dashed', classes);
  assert.equal(classStyle('border').borderStyle, undefined);
  assert.equal(classStyle('border-2 border-dashed md:border-dotted', env('light', 900)).borderStyle, 'dotted');
  assert.equal(node('column', { className: 'border border-dotted' }).borderStyle, 'dotted');
  assert.throws(() => classStyle('[border-top-style:dashed]'), /per-edge border styles/);
});

test('radii: infinite radius clamps and corners cascade under shorthands', async () => {
  await use(['rounded-full', 'rounded-none', 'rounded-3xl', 'rounded-br-md', 'rounded-t-none', 'rounded-br-full', '[width:calc(infinity*1px)]']);
  assert.deepEqual(classStyle('rounded-full'), { borderRadius: 16384 });
  assert.deepEqual(classStyle('rounded-none'), { borderRadius: 0 });
  assert.deepEqual(classStyle('rounded-3xl rounded-br-md'), { borderRadius: 24, borderBottomRightRadius: 6 });
  assert.deepEqual(classStyle('rounded-br-md rounded-3xl'), classStyle('rounded-3xl rounded-br-md'));
  assert.deepEqual(classStyle('rounded-3xl rounded-t-none'), { borderRadius: 24, borderTopLeftRadius: 0, borderTopRightRadius: 0 });
  assert.equal(classStyle('rounded-br-full').borderBottomRightRadius, 16384);
  const inline = node('column', { className: 'rounded-3xl rounded-br-md', style: { borderRadius: 8 } });
  assert.deepEqual([inline.radius, inline.radiusBottomRight], [8, 8], 'an inline shorthand resets class longhands');
  assert.throws(() => classStyle('[width:calc(infinity*1px)]'), /UI_CLASS/);
});

test('real Tailwind utilities lower to native dimensions, typography and colors', async () => {
  await use(['p-6', 'gap-2', 'text-sm', 'font-semibold', 'rounded-xl', 'bg-blue-500/80', 'opacity-50']);
  assert.deepEqual(classStyle('p-6 gap-2 text-sm font-semibold rounded-xl opacity-50'), {
    paddingTop: 24, paddingRight: 24, paddingBottom: 24, paddingLeft: 24, gap: 8, fontSize: 14, lineHeight: 1.25 / 0.875,
    fontWeight: 600, borderRadius: 12, opacity: .5,
  });
  assert.equal(node('text', { className: 'text-sm' }).lineHeight, 20);
  assert.match(classStyle('bg-blue-500/80').backgroundColor, /^#[0-9a-f]{6}cc$/);
});

test('cascade follows Tailwind order and responds to viewport and appearance', async () => {
  await use(['gap-2', 'md:gap-4', 'bg-white', 'dark:bg-black', 'p-4', 'p-2', 'pl-1', 'md:p-8']);
  assert.equal(classStyle('p-4 p-2').paddingLeft, 16);
  assert.equal(classStyle('p-2 p-4').paddingLeft, 16);
  assert.deepEqual(classStyle('gap-2 md:gap-4 bg-white dark:bg-black', env('light', 767)), { gap: 8, backgroundColor: '#ffffffff' });
  assert.deepEqual(classStyle('gap-2 md:gap-4 bg-white dark:bg-black', env('dark', 768)), { gap: 16, backgroundColor: '#000000ff' });
  assert.equal(classStyle('pl-1 md:p-8', env('light', 900)).paddingLeft, 32);
});

test('custom theme tokens and arbitrary values retain Tailwind semantics', async () => {
  await use(['bg-primary', 'rounded-xl', 'h-[50px]', 'text-[17px]'], '@theme { --color-primary: #007aff; --radius-xl: 16px; }');
  assert.deepEqual(classStyle('bg-primary rounded-xl h-[50px] text-[17px]'), { backgroundColor: '#007affff', borderRadius: 16, height: 50, fontSize: 17 });
});

test('unsupported rendering and missing dynamic candidates fail visibly', async () => {
  const rules = await compileUtilities(new Set(['backdrop-invert', 'transition', 'skew-y-3', 'group-hover:underline', 'p-4']));
  const errors = Object.fromEntries(rules.filter(rule => rule.error).map(rule => [rule.name, rule.error]));
  assert.deepEqual(Object.keys(errors).sort(), ['backdrop-invert', 'group-hover:underline', 'skew-y-3', 'transition']);
  assert.ok(Object.values(errors).every(error => error.category === 'unsupported'));
  assert.match(errors.transition.message, /transition-property \(the native renderer has no transitions\)/);
  configureClasses(rules);
  for (const name of ['backdrop-invert', 'transition', 'p-8']) assert.throws(() => classStyle(name), /UI_CLASS/);
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
  await use(['bg-surface/80', 'text-foreground', 'text-primary', 'dark:text-white'], css);
  assert.deepEqual(classStyle('bg-surface/80 text-foreground', { theme: 'light' }), { backgroundColor: '#ffffffcc', color: '#1c1c1eff' });
  assert.deepEqual(classStyle('bg-surface/80 text-foreground'), { backgroundColor: '#ffffffcc', color: '#1c1c1eff' });
  assert.deepEqual(classStyle('bg-surface/80 text-foreground', { theme: 'dark' }), { backgroundColor: '#1f1f22cc', color: '#f2f2f7ff' });
  assert.equal(classStyle('text-primary', { theme: 'light' }).color, '#007affff');
  assert.equal(classStyle('text-primary', { theme: 'dark' }).color, '#0a84ffff');
  assert.equal(classStyle('dark:text-white text-primary', { theme: 'dark' }).color, '#ffffffff');
});

test('palette rules obey source order and diagnose invalid variables in either appearance', async () => {
  await use(['bg-surface'], `
    @theme inline { --color-surface: var(--fill); }
    [data-theme="dark"] { --fill: black; }
    :root { --fill: white; }
    [data-theme="light"] { --fill: red; }
  `);
  assert.equal(classStyle('bg-surface', { theme: 'dark' }).backgroundColor, '#ffffffff');
  assert.equal(classStyle('bg-surface', { theme: 'light' }).backgroundColor, '#ff0000ff');
  for (const value of ['var(--missing)', 'var(--fill)']) {
    await use(['bg-surface'], `@theme inline { --color-surface: var(--fill); } :root { --fill: white; } [data-theme="dark"] { --fill: ${value}; }`);
    assert.throws(() => classStyle('bg-surface', { theme: 'light' }), /UI_CLASS.*(?:unresolved|cyclic)/);
  }
  for (const css of ['[data-theme="dark"] { color-scheme: light; }', '[data-theme="dark"] { --fill: black !important; }'])
    await assert.rejects(compileUtilities(new Set(['p-4']), css), /UI_CSS/);
  await assert.rejects(compileUtilities(new Set(['p-4']), '@theme { @keyframes spin {} }'), /UI_ANIMATION/);
});

test('relative line height and tracking follow font size across components and lists', async () => {
  await use(['text-sm', 'leading-normal', 'leading-5', 'text-[20px]', 'text-[21px]', 'text-[10px]', 'tracking-widest', 'tracking-tight', 'tracking-normal', 'tracking-[-1px]']);
  assert.equal(node('text', { className: 'text-sm', style: { fontSize: 21 } }).lineHeight, 30);
  const nested = rendered(() => h('column', { className: 'text-sm' }, h('text', { className: 'text-[20px]' }, 'child')));
  assert.ok(Math.abs(nested.children[0].props.lineHeight - 20 * 20 / 14) < 1e-9);
  assert.equal(node('text', { className: 'leading-5', style: { fontSize: 21 } }).lineHeight, 20);
  assert.equal(node('text', { className: 'leading-normal', style: { fontSize: 20 } }).lineHeight, 30);
  assert.equal(node('text', { className: 'text-sm', style: { lineHeight: '40px' } }).lineHeight, 40);
  const Label = ({ children }) => h('text', { className: 'text-[21px]' }, children);
  const texts = decode(describe(() => h('column', { className: 'text-sm' }, h(Label, {}, 'child'), h('list', { count: 1, row: () => h(Label, {}, 'row') })), {}))
    .filter(record => record.tag === 'text');
  assert.deepEqual(texts.map(text => text.props.lineHeight), [30, 30]);
  assert.equal(node('text', { className: 'text-[10px] tracking-widest' }).letterSpacing, 1);
  assert.equal(node('text', { className: 'text-[10px] tracking-tight' }).letterSpacing, -0.25);
  assert.deepEqual([classStyle('tracking-normal').letterSpacing, classStyle('tracking-[-1px]').letterSpacing], ['0em', -1]);
});

test('padding, borders and word breaking keep shorthand, axis, edge and variant semantics', async () => {
  await use(['p-2', 'px-6', 'py-3', 'pl-1', 'md:p-4', 'dark:px-3', '[padding:1px_2px_3px_4px]', 'border', 'border-4', 'border-t-2', 'border-x-8',
    'border-blue-500/50', 'border-current', 'border-dashed', 'border-none', 'break-all', 'break-normal', 'wrap-anywhere', 'h-auto', 'min-h-[42px]', 'max-h-[120px]']);
  const names = 'pl-1 px-6 p-2 py-3 md:p-4 dark:px-3';
  assert.deepEqual(classStyle(names, env('light', 600)), { paddingTop: 12, paddingRight: 24, paddingBottom: 12, paddingLeft: 4 });
  assert.deepEqual(classStyle(names, env('light', 800)), { paddingTop: 16, paddingRight: 16, paddingBottom: 16, paddingLeft: 16 });
  assert.deepEqual(classStyle(names, env('dark', 800)), { paddingTop: 16, paddingRight: 12, paddingBottom: 16, paddingLeft: 12 });
  assert.deepEqual(classStyle('[padding:1px_2px_3px_4px]'), { paddingTop: 1, paddingRight: 2, paddingBottom: 3, paddingLeft: 4 });
  const inline = node('column', { className: 'px-6 py-3', style: { paddingLeft: 7, padding: 5 } });
  assert.deepEqual([inline.paddingTop, inline.paddingRight, inline.paddingLeft], [5, 5, 7]);
  assert.deepEqual(classStyle('border'), { borderTopWidth: 1, borderRightWidth: 1, borderBottomWidth: 1, borderLeftWidth: 1 });
  assert.equal(classStyle('border-4 border-t-2').borderTopWidth, 2);
  assert.deepEqual(classStyle('border-x-8'), { borderLeftWidth: 8, borderRightWidth: 8 });
  assert.equal(classStyle('border-current').borderColor, 'currentColor');
  assert.match(classStyle('border-blue-500/50').borderColor, /80$/);
  assert.throws(() => classStyle('border-none'), /UI_CLASS/);
  assert.deepEqual(classStyle('break-normal'), { wordBreak: 'normal', overflowWrap: 'normal' });
  assert.equal(classStyle('wrap-anywhere').overflowWrap, 'anywhere');
  const textarea = rendered(() => h('textarea', { className: 'h-auto min-h-[42px] max-h-[120px]', value: 'hello' })).props;
  assert.deepEqual([textarea.heightAuto, textarea.height, textarea.heightPercent, textarea.minHeight], [1, undefined, undefined, 42]);
});

test('backdrop variables compose through CSS order, variants and disabling', async () => {
  const names = ['backdrop-blur-xl', 'backdrop-blur-none', 'backdrop-saturate-180', 'md:backdrop-filter-none', 'dark:backdrop-blur-sm', 'backdrop-brightness-125',
    'backdrop-blur-[65px]', 'backdrop-saturate-[1700%]', '[backdrop-filter:blur(20px)_saturate(180%)]', '[backdrop-filter:saturate(180%)_blur(20px)]'];
  await use(names);
  const classes = 'backdrop-blur-xl backdrop-saturate-180 md:backdrop-filter-none dark:backdrop-blur-sm';
  assert.deepEqual(classStyle(classes, env()), { backdropFilter: 'blur(24px) saturate(1.8)' });
  assert.deepEqual(classStyle(classes.split(' ').reverse().join(' '), env()), classStyle(classes, env()));
  assert.deepEqual(classStyle(classes, env('dark')), { backdropFilter: 'blur(8px) saturate(1.8)' });
  assert.deepEqual(classStyle(classes, env('light', 900)), {});
  assert.deepEqual(classStyle('backdrop-blur-none'), {});
  const filter = node('column', { className: '[backdrop-filter:blur(20px)_saturate(180%)]' });
  assert.deepEqual([filter.backdropBlur, filter.backdropSaturation], [20, 1.8]);
  for (const name of ['backdrop-brightness-125', 'backdrop-blur-[65px]', 'backdrop-saturate-[1700%]', '[backdrop-filter:saturate(180%)_blur(20px)]'])
    assert.throws(() => classStyle(name), /UI_CLASS/);
});

test('placeholder color remains separate from text across theme and viewport', async () => {
  await use(['text-blue-500', 'placeholder:text-[#123456]/30', 'dark:placeholder:text-[#abcdef]', 'md:placeholder:text-red-500', 'placeholder:font-bold', 'placeholder:bg-red-500']);
  const names = 'text-blue-500 placeholder:text-[#123456]/30 dark:placeholder:text-[#abcdef] md:placeholder:text-red-500';
  const light = classStyle(names, env()), dark = classStyle(names, env('dark'));
  assert.equal(light.placeholderColor, '#1234564d');
  assert.equal(dark.placeholderColor, '#abcdefff');
  assert.equal(light.color, dark.color);
  assert.notEqual(classStyle(names, env('light', 900)).placeholderColor, light.placeholderColor);
  for (const name of ['placeholder:font-bold', 'placeholder:bg-red-500']) assert.throws(() => classStyle(name), /placeholder supports only color/);
});

test('editor focus paint preserves cascade order without affecting base style', async () => {
  await use(['bg-white', 'focus:bg-red-500', 'dark:focus:bg-blue-500', 'focus:shadow-lg', 'focus:placeholder:text-[#123456]', 'focus:p-4']);
  const classes = 'bg-white focus:bg-red-500 dark:focus:bg-blue-500 focus:shadow-lg';
  assert.deepEqual(classStyle(classes, env()), { backgroundColor: '#ffffffff' });
  const focus = resolveClasses(classes, env()).focus;
  assert.ok(focus.boxShadow.length);
  assert.notEqual(focus.backgroundColor, resolveClasses(classes, env('dark')).focus.backgroundColor);
  assert.deepEqual(focus, resolveClasses(classes.split(' ').reverse().join(' '), env()).focus);
  assert.throws(() => classStyle('focus:placeholder:text-[#123456]'), /UI_CLASS/);
  assert.throws(() => describe(() => h('input', { className: 'focus:p-4' }), {}), /focus: supports paint properties only/);
  assert.throws(() => describe(() => h('button', { className: 'focus:bg-red-500' }, 'x'), {}), /focus: requires an editor/);
});
