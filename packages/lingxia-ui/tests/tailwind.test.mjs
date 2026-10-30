import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compileUtilities } from '../compiler/tailwind.mjs';
import { configureClasses, resolveClasses } from '../src/style/classes.js';
import { describe, h, listTag, prepare, discard } from '../src/internal.js';
import { rendered, decode } from './support/wire.mjs';

// Tailwind rules lower to CSS-named style; rendering then lowers that to the native box.
const classStyle = (name, env) => resolveClasses(name, env)?.base ?? {};
const use = async (names, theme) => configureClasses(await compileUtilities(new Set(names), theme));
const env = (theme = 'light', width = 400) => ({ theme, viewport: { width, height: 800 } });
const node = (tag, props, view = {}) => rendered(() => h(tag, props, ...(tag === 'text' ? ['x'] : [])), view).props;

test('flex direction follows utility cascade, responsive rules and explicit style', async () => {
  await use(['flex', 'inline-flex', 'flex-col', 'md:flex-row', 'flex-row-reverse', 'flex-col-reverse']);
  assert.deepEqual(classStyle('flex flex-col md:flex-row', env('light', 360)), { display: 'flex', flexDirection: 'column' });
  assert.deepEqual(classStyle('md:flex-row flex-col flex', env('light', 900)), { display: 'flex', flexDirection: 'row' });
  assert.equal(node('view', { className: 'flex-col', style: { flexDirection: 'row-reverse' } }).flexDirection, 'row-reverse');
  assert.equal(classStyle('flex-col-reverse').flexDirection, 'column-reverse');
  const view = rendered(() => h('view', { className: 'flex flex-col' }, h('text', {}, 'Hello')));
  assert.deepEqual([view.tag, view.props.display, view.props.flexDirection], ['column', 'flex', 'column']);
  // As in CSS, a `flex` view lays out a row; a `<column>` stays a column.
  assert.equal(rendered(() => h('view', { className: 'flex' }, h('text', {}, 'a'))).props.flexDirection, 'row');
  assert.equal(rendered(() => h('view', { className: 'inline-flex' }, h('text', {}, 'a'))).props.flexDirection, 'row');
  assert.equal(rendered(() => h('column', { className: 'flex' }, h('text', {}, 'a'))).props.flexDirection, undefined);
  assert.throws(() => describe(() => h('grid', { className: 'flex-col', style: { gridTemplateColumns: 2 } }), {}), /UI_CLASS_ELEMENT: flex-col does not apply to <grid>; use grid-flow-col/);
});

test('display utilities, fixed positioning and dynamic viewport units lower to defined native semantics', async () => {
  await use(['hidden', 'block', 'inline-flex', 'inline-block', 'md:hidden', 'contents', 'table', 'fixed', 'sticky', 'h-dvh', 'min-h-svh', 'max-h-lvh', 'w-dvw', 'h-[50dvh]']);
  assert.deepEqual(classStyle('hidden'), { display: 'none' });
  assert.deepEqual(classStyle('block md:hidden', env('light', 900)), { display: 'none' });
  for (const name of ['block', 'inline-flex', 'inline-block']) assert.equal(classStyle(name).display, name);
  assert.equal(node('text', { className: 'hidden' }).display, 'none');
  assert.equal(node('image', { src: 'a.png', className: 'hidden' }).display, 'none');
  for (const name of ['contents', 'table']) assert.throws(() => classStyle(name), /UI_CLASS/);
  assert.equal(classStyle('sticky').position, 'sticky');
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
  assert.throws(() => classStyle('transform'), /apply without it/);
  for (const name of ['skew-x-6', 'rotate-x-45']) assert.throws(() => classStyle(name), /only translate, rotate and scale/);
  assert.deepEqual(JSON.parse(node('column', { className: 'translate-x-2 rotate-45 -scale-x-100 origin-top-left' }).transform),
    { translate: [8, 0], rotate: 45, scale: [-1, 1], origin: [0, 0] });
  // Transforms are paint, so pressed states may scale.
  assert.deepEqual(JSON.parse(rendered(() => h('button', { className: 'active:scale-95' }, 'Go')).props.pressedStyle.transform), { scale: [0.95, 0.95] });
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
  assert.throws(() => describe(() => h('column', { className: 'overscroll-contain' }), {}), /UI_CLASS_ELEMENT: overscroll-contain does not apply to <column>; it applies to <scroll>, <List>/);
  assert.deepEqual(['select-none', 'select-text', 'select-all'].map(name => classStyle(name).userSelect), ['none', 'text', 'all']);
  assert.equal(node('text', { className: 'select-text' }).userSelect, 'text');
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
  assert.throws(() => describe(() => h('column', { dir: 'sideways' }), {}), /UI_PROP: dir/);
  assert.equal(rendered(() => h('input', { value: '', dir: 'auto' })).props.direction, 'auto');
});

test('structural variants match sibling position, list rows and emptiness; DOM-state variants name the TS alternative', async () => {
  await use(['first:pt-1', 'last:pb-2', 'odd:bg-surface', 'even:bg-text', 'only:p-4', 'empty:hidden', 'first:hover:p-3', 'md:first:pt-3',
    'has-[input]:p-2', 'data-[state=open]:p-2', 'aria-checked:p-2']);
  const tree = rendered(() => h('column', {}, ...['a', 'b', 'c'].map(key => h('text', { key, className: 'first:pt-1 last:pb-2 odd:bg-surface even:bg-text' }, key))));
  const rows = tree.children.map(child => [child.props.paddingTop, child.props.paddingBottom, child.props.background]);
  assert.deepEqual(rows, [[4, undefined, 'surface'], [undefined, undefined, 'text'], [undefined, 8, 'surface']]);
  assert.equal(rendered(() => h('column', {}, h('text', { className: 'only:p-4' }, 'x'))).children[0].props.paddingTop, 16);
  assert.equal(rendered(() => h('column', {}, false, h('text', { className: 'only:p-4' }, 'x'), null)).children[0].props.paddingTop, 16);
  // Holes keep their slot in identity but never count as siblings.
  const holes = rendered(() => h('column', {}, false, h('text', { className: 'first:pt-1' }, 'a'), null, h('text', { className: 'last:pb-2' }, 'b')));
  assert.deepEqual(holes.children.map(child => [child.props.paddingTop, child.props.paddingBottom, child.props.childSlot]),
    [[4, undefined, '1'], [undefined, 8, '3']]);
  assert.equal(rendered(() => h('column', {}, h('text', {}, 'a'), h('text', {}, 'b'))).children[1].props.childSlot, undefined,
    'a child at its rendered position sends no slot');
  assert.equal(rendered(() => h('column', { className: 'empty:hidden' })).props.display, 'none');
  assert.equal(rendered(() => h('column', { className: 'empty:hidden' }, h('text', {}, 'x'))).props.display, undefined);
  const page = () => h(listTag, { count: 3, row: i => h('text', { className: 'first:pt-1 last:pb-2' }, String(i)) });
  const texts = decode(prepare(page, {}, { root: { start: 0, end: 3, leading: 0, trailing: 0 } })).filter(node => node.tag === 'text').map(node => node.props);
  discard();
  assert.deepEqual([texts[0].paddingTop, texts[2].paddingBottom, texts[1].paddingTop], [4, 8, undefined]);
  for (const [name, hint] of [['has-[input]:p-2', /has-\* supports :checked, :disabled and :focus of a descendant/], ['aria-checked:p-2', /set aria-\* props/]])
    assert.throws(() => classStyle(name), hint);
  assert.deepEqual(classStyle('data-[state=open]:p-2'), {}, 'no data-state prop: the rule does not apply');
});

test('horizontal overflow lowers only onto scroll containers', async () => {
  await use(['overflow-x-auto', 'md:overflow-x-auto', 'overflow-hidden']);
  assert.deepEqual(classStyle('overflow-x-auto'), { overflowX: 'auto' });
  assert.deepEqual(classStyle('md:overflow-x-auto', env('light', 320)), {});
  assert.deepEqual(classStyle('md:overflow-x-auto', env('light', 900)), { overflowX: 'auto' });
  assert.equal(node('scroll', { className: 'overflow-x-auto' }).scrollAxis, 'horizontal');
  assert.throws(() => describe(() => h('row', { className: 'overflow-x-auto' }), {}), /UI_CLASS_ELEMENT: overflow-x-auto does not apply to <row>; it applies to <scroll>/);
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
  await use(['p-4', 'leading-relaxed', 'tracking-wider', 'shadow-sm', 'text-danger', 'dark:tracking-widest', 'md:p-8']);
  const classes = 'p-4 leading-relaxed tracking-wider shadow-sm text-danger dark:tracking-widest md:p-8';
  const first = resolveClasses(classes, env());
  first.base.boxShadow[0].color = '#123456'; first.base.paddingTop = 900;
  const second = node('text', { className: classes, style: { fontSize: 20 } }, { env: env() });
  assert.equal(second.paddingTop, 16); assert.equal(second.lineHeight, 32.5); assert.equal(second.letterSpacing, 1);
  assert.notEqual(JSON.parse(second.boxShadow)[0].color, '#123456');
  const dark = classStyle(classes, env('dark', 900));
  assert.equal(dark.paddingTop, 32); assert.notEqual(dark.letterSpacing, classStyle(classes, env()).letterSpacing);
  assert.equal(dark.color, 'danger');
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
  // The class written later wins a conflict, as tailwind-merge resolves it.
  assert.equal(classStyle('flex-1 basis-auto').flexBasis, 'auto');
  assert.equal(classStyle('basis-auto flex-1').flexBasis, '0%');
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
  await use(['m-2', 'mx-auto', '-mt-1', 'relative', 'absolute', 'inset-0', 'inset-x-4', 'top-1/2', 'z-10', 'truncate', 'line-clamp-3', 'grid-cols-3', 'object-cover', 'object-top-left', 'object-bottom', 'object-[25%_75%]']);
  assert.deepEqual(classStyle('m-2 mx-auto -mt-1'), { marginTop: -4, marginRight: 'auto', marginBottom: 8, marginLeft: 'auto' });
  assert.deepEqual(classStyle('absolute inset-0 inset-x-4 z-10'), { position: 'absolute', top: 0, right: 16, bottom: 0, left: 16, zIndex: 10 });
  assert.equal(classStyle('top-1/2').top, '50%');
  assert.deepEqual(classStyle('truncate'), { overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' });
  assert.deepEqual(classStyle('line-clamp-3'), { overflow: 'hidden', lineClamp: 3 });
  assert.equal(node('text', { className: 'line-clamp-3' }).textLines, 3);
  assert.equal(node('grid', { className: 'grid-cols-3' }).gridTemplateColumns, 'repeat(3, minmax(0, 1fr))');
  assert.equal(node('image', { src: 'a.png', className: 'object-cover' }).imageFit, 'cover');
  assert.equal(node('image', { src: 'a.png', className: 'object-top-left' }).imagePosition, '[0,0]');
  assert.equal(node('image', { src: 'a.png', className: 'object-bottom' }).imagePosition, '[0.5,1]');
  assert.equal(node('image', { src: 'a.png', className: 'object-[25%_75%]' }).imagePosition, '[0.25,0.75]');
  assert.throws(() => describe(() => h('column', { className: 'line-clamp-3' }), {}), /UI_CLASS_ELEMENT: line-clamp-3 does not apply to <column>; it applies to <text>/);
});

test('space and divide utilities target non-last children', async () => {
  await use(['space-y-2', 'space-x-3', 'divide-y', 'divide-line', 'gap-x-2', 'gap-y-4']);
  assert.deepEqual(resolveClasses('space-y-2').children, { marginTop: 0, marginBottom: 8 });
  assert.deepEqual(resolveClasses('space-x-3').children, { marginInlineStart: 0, marginInlineEnd: 12 });
  const list = rendered(() => h('column', { className: 'space-y-2 divide-y divide-line' }, h('text', {}, 'a'), h('text', {}, 'b')));
  assert.deepEqual(list.children.map(child => [child.props.marginBottom, child.props.borderBottomWidth]), [[8, 1], [undefined, undefined]]);
  assert.equal(list.children[0].props.borderColor, 'line');
  assert.deepEqual(classStyle('gap-x-2 gap-y-4'), { columnGap: 8, rowGap: 16 });
});

test('state variants lower to state styles and disabled replaces the cascade', async () => {
  await use(['bg-surface', 'active:bg-surface-sunken', 'hover:bg-surface-sunken', 'disabled:opacity-50', 'focus:bg-danger']);
  const states = resolveClasses('bg-surface active:bg-surface-sunken hover:bg-surface-sunken disabled:opacity-50', { pointer: 'fine' });
  assert.deepEqual(Object.keys(states.active), ['backgroundColor']);
  assert.deepEqual(Object.keys(states.hover), ['backgroundColor']);
  assert.equal(states.disabled.opacity, 0.5);
  assert.equal(resolveClasses('hover:bg-surface-sunken', { pointer: 'coarse' }).hover, undefined);
  const button = rendered(() => h('button', { className: 'bg-surface active:bg-surface-sunken disabled:opacity-50', disabled: true }, 'Save')).props;
  assert.deepEqual([button.opacity, button.pressedStyle, button.enabled], [0.5, undefined, 0]);
  assert.ok(rendered(() => h('input', { className: 'focus:bg-danger' })).props.focusStyle.background);
});

test('starting and exit variants lower to presence states', async () => {
  await use(['opacity-100', 'starting:opacity-0', 'exit:opacity-0', 'exit:translate-y-2', 'starting:p-2']);
  const states = resolveClasses('opacity-100 starting:opacity-0 exit:opacity-0 exit:translate-y-2');
  assert.equal(states.starting.opacity, 0);
  assert.equal(states.exit.opacity, 0);
  const panel = rendered(() => h('column', { className: 'opacity-100 starting:opacity-0 exit:opacity-0' })).props;
  assert.deepEqual([panel.startingStyle, panel.exitStyle], ['[58,0]', '[58,0]'], 'opacity 0, wire-encoded');
  assert.throws(() => rendered(() => h('column', { className: 'starting:p-2' })), /starting: supports paint properties only, not paddingTop/);
});

test('transition-layout adds layout motion to whatever transition-* sets', async () => {
  await use(['transition-layout', 'transition-opacity', 'duration-300', 'transition-none']);
  assert.deepEqual(classStyle('transition-layout').transition.properties, ['layout']);
  const both = classStyle('transition-opacity transition-layout duration-300').transition;
  assert.deepEqual([both.properties, both.durationMs], [['opacity', 'layout'], 300]);
  assert.equal(classStyle('transition-none').transition, 'none');
});

test('bg-clip-text paints a text background through its glyphs, on <text> only', async () => {
  await use(['bg-clip-text', 'bg-clip-border', 'text-transparent', 'bg-linear-to-r', 'from-accent', 'to-danger']);
  assert.deepEqual(classStyle('bg-clip-text'), { backgroundClip: 'text' });
  const text = rendered(() => h('text', { className: 'bg-clip-text text-transparent bg-linear-to-r from-accent to-danger' }, 'Thinking')).props;
  assert.equal(text.backgroundClip, 'text');
  assert.ok(text.backgroundGradient);
  assert.throws(() => describe(() => h('column', { className: 'bg-clip-text' }), {}), /UI_CLASS_ELEMENT: bg-clip-text does not apply to <column>; it applies to <text>/);
});

test('mask edge fades and linear mask-image lower to intersected alpha fades', async () => {
  await use(['mask-t-from-80%', 'mask-b-to-4', 'mask-[linear-gradient(to_bottom,transparent_0,#000_1rem)]', 'mask-none', 'mask-radial-from-50%', 'mask-[radial-gradient(#000,transparent)]']);
  assert.deepEqual(classStyle('mask-t-from-80% mask-b-to-4').maskImage, [
    { to: 'top', stops: [{ alpha: 1, offset: 0.8 }, { alpha: 0, offset: 1 }] },
    { to: 'bottom', stops: [{ alpha: 1, offset: 0 }, { alpha: 0, px: 16 }] },
  ]);
  assert.deepEqual(classStyle('mask-[linear-gradient(to_bottom,transparent_0,#000_1rem)]').maskImage, [{ to: 'bottom', stops: [{ alpha: 0, px: 0 }, { alpha: 1, px: 16 }] }]);
  assert.equal(classStyle('mask-t-from-80% mask-none').maskImage, 'none');
  const wire = rendered(() => h('column', { className: 'mask-b-to-4' })).props.maskImage;
  assert.deepEqual(JSON.parse(wire), [{ to: 'bottom', stops: [{ offset: 0, alpha: 1 }, { px: 16, alpha: 0 }] }]);
  assert.throws(() => classStyle('mask-radial-from-50%'), /radial and conic masks/);
  assert.throws(() => classStyle('mask-[radial-gradient(#000,transparent)]'), /only linear-gradient fades/);
});

test('blur utilities and filter: blur() lower to a subtree blur; other filters are refused', async () => {
  await use(['blur-md', 'blur-[10px]', 'blur-none', '[filter:blur(4px)]', 'brightness-50', '[filter:grayscale(1)]', 'blur-[100px]']);
  assert.deepEqual(classStyle('blur-md'), { filter: 'blur(12px)' });
  assert.equal(rendered(() => h('column', { className: 'blur-[10px]' })).props.filterBlur, 10);
  assert.equal(rendered(() => h('column', { className: 'blur-none' })).props.filterBlur, 0);
  assert.equal(rendered(() => h('column', { className: '[filter:blur(4px)]' })).props.filterBlur, 4);
  assert.throws(() => classStyle('brightness-50'), /sets nothing/);
  assert.throws(() => classStyle('[filter:grayscale(1)]'), /only blur/);
  assert.throws(() => describe(() => h('column', { className: 'blur-[100px]' }), {}), /UI_STYLE_VALUE: filter/);
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
  assert.deepEqual(classStyle('font-serif'), { fontFamily: 'serif' });
  assert.throws(() => classStyle('[font-family:Custom,sans-serif]'), /declare a bundled face with @font-face/);
});

test('outer shadows preserve layers, color overrides, variants and CSS order', async () => {
  const names = ['shadow-sm', 'shadow-lg', 'shadow-none', 'shadow-danger/40', 'md:shadow-none'];
  const rules = await compileUtilities(new Set(names));
  assert.deepEqual(rules.filter(rule => rule.error), []);
  configureClasses(rules);
  assert.deepEqual(classStyle('shadow-lg').boxShadow, [
    { offsetX: 0, offsetY: 10, blur: 15, spread: -3, color: 'shadow/10' },
    { offsetX: 0, offsetY: 4, blur: 6, spread: -4, color: 'shadow/10' },
  ]);
  const classes = 'shadow-sm shadow-danger/40 md:shadow-none';
  const style = classStyle(classes, env());
  assert.deepEqual(style, classStyle(classes.split(' ').reverse().join(' '), env()));
  assert.equal(style.boxShadow.length, 2);
  assert.equal(style.boxShadow[0].color, 'danger/40');
  assert.equal(style.boxShadow[0].color, style.boxShadow[1].color);
  assert.equal(classStyle(classes, env('light', 800)).boxShadow[0].color, '#00000000');
  assert.deepEqual(classStyle('shadow-danger/40'), {});
  const [palette, dark] = await compileUtilities(new Set(['shadow-red-500', 'dark:shadow-accent']));
  assert.deepEqual([palette.error.category, dark.error.message], ['color', 'roles follow appearance; drop dark:']);
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
  await use(['shadow-card', 'shadow-[var(--elev)]', '[box-shadow:var(--elev)]', 'shadow-[var(--nine)]', 'shadow-danger'], theme);
  assert.deepEqual(classStyle('shadow-card', env()).boxShadow.map(layer => layer.offsetY), [1, 4]);
  assert.deepEqual(classStyle('shadow-card', env('dark')).boxShadow, [{ offsetX: 0, offsetY: 0, blur: 0, spread: 1, color: '#ffffff1a' }]);
  // Black layers take the theme's shadow ink at their own alpha.
  assert.deepEqual(classStyle('shadow-[var(--elev)]').boxShadow.map(layer => layer.color), ['shadow/13', 'shadow/20']);
  assert.deepEqual(classStyle('[box-shadow:var(--elev)]').boxShadow.length, 2);
  // A layer Tailwind did not wrap in --tw-shadow-color keeps its own color, as in CSS.
  assert.equal(classStyle('shadow-[var(--elev)] shadow-danger').boxShadow[0].color, 'shadow/13');
  assert.equal(classStyle('shadow-card', env()).boxShadow[1].color.startsWith('#'), true, 'a tinted layer keeps its colour');
  assert.throws(() => classStyle('shadow-[var(--nine)]'), /too many box shadows/);
});

test('outer rings compose before shadows and fixed shadows replace the composition', async () => {
  await use(['ring-2', 'ring-0', 'ring-accent/30', 'shadow-lg', 'shadow-none', 'focus:ring-2', 'focus:ring-accent/30', 'ring-offset-2', 'ring-inset', '[box-shadow:none]']);
  const style = classStyle('ring-2 ring-accent/30 shadow-lg');
  assert.equal(style.boxShadow.length, 3); assert.equal(style.boxShadow[0].spread, 2); assert.equal(style.boxShadow[0].color, 'accent/30');
  assert.equal(classStyle('ring-2 shadow-none').boxShadow[0].spread, 2);
  assert.deepEqual(classStyle('ring-0').boxShadow, []);
  assert.deepEqual(classStyle('ring-2 [box-shadow:none]').boxShadow, []);
  assert.deepEqual(classStyle('focus:ring-2 focus:ring-accent/30'), {});
  assert.equal(resolveClasses('focus:ring-2 focus:ring-accent/30').focus.boxShadow[0].spread, 2);
  for (const name of ['ring-offset-2', 'ring-inset']) assert.throws(() => classStyle(name), /unsupported shadow property/);
});

test('gradient utilities compose across classes using Tailwind cascade and theme', async () => {
  const names = ['bg-linear-to-br', 'bg-linear-to-r/srgb', 'from-danger', 'via-success', 'to-accent', 'from-80%', 'via-10%', 'to-20%', 'md:bg-none', 'bg-surface'];
  const rules = await compileUtilities(new Set(names));
  assert.deepEqual(rules.filter(rule => rule.error), []);
  configureClasses(rules);
  const classes = 'bg-surface bg-linear-to-br from-danger via-success to-accent from-80% via-10% to-20% md:bg-none';
  const style = classStyle(classes, env());
  assert.deepEqual(style, classStyle(classes.split(' ').reverse().join(' '), env()));
  assert.equal(style.backgroundColor, 'surface');
  assert.equal(style.backgroundImage.direction, 'bottom-right');
  assert.equal(style.backgroundImage.colorSpace, 'oklab');
  assert.deepEqual(style.backgroundImage.stops.map(stop => stop.offset), [.8, .8, .8]);
  assert.equal(classStyle(classes, env('dark')).backgroundImage.stops[0].color, 'danger');
  assert.equal(classStyle(classes, env('light', 800)).backgroundImage, 'none');
  assert.equal(classStyle('bg-linear-to-r/srgb from-danger to-accent').backgroundImage.colorSpace, 'srgb');
  assert.deepEqual(classStyle('from-danger'), {});
});

test('gradient defaults, transparency, negative stops and unsupported spaces are explicit', async () => {
  await use(['bg-linear-45', 'from-danger/50', 'to-accent', 'from-[-10%]', 'to-[120%]', 'bg-linear-to-r/hsl', 'via-[12px]']);
  const gradient = classStyle('bg-linear-45 from-danger/50 from-[-10%] to-accent to-[120%]').backgroundImage;
  assert.equal(gradient.angle, 45);
  assert.equal(gradient.stops[0].offset, -.1);
  assert.equal(gradient.stops[1].offset, 1.2);
  assert.equal(gradient.stops[0].color, 'danger/50');
  assert.equal(classStyle('bg-linear-45 from-danger/50').backgroundImage.stops[1].color, '#00000000');
  assert.equal(classStyle('bg-linear-45').backgroundImage, 'none');
  for (const name of ['bg-linear-to-r/hsl', 'via-[12px]']) assert.throws(() => classStyle(name), /UI_CLASS/);
});

test('radial, conic and complete CSS gradients lower from utilities, arbitrary values and theme tokens', async () => {
  const theme = `@theme { --gradient-hero: linear-gradient(135deg, #ffffff, #000000 80%); }
    :root { --glow: radial-gradient(circle at top, #ff000080, transparent 70%); }`;
  await use(['bg-radial', 'bg-radial-[at_25%_75%]', 'bg-conic-180', 'bg-conic', 'from-danger', 'to-accent',
    'bg-[radial-gradient(circle_closest-side_at_left_top,#fff,#000_50%,#f00)]', 'bg-[image:var(--glow)]', 'bg-(image:--gradient-hero)',
    'bg-[conic-gradient(from_90deg,#fff_0deg,#000_0.5turn)]', 'bg-[repeating-linear-gradient(#fff,#000)]', 'bg-[radial-gradient(#fff_10px,#000)]',
    'bg-[linear-gradient(at_top,#fff,#000)]', 'bg-linear-45'], theme);
  const radial = classStyle('bg-radial-[at_25%_75%] from-danger to-accent').backgroundImage;
  assert.deepEqual([radial.kind, radial.at, radial.stops.length], ['radial', [.25, .75], 2]);
  assert.equal(classStyle('bg-radial from-danger to-accent').backgroundImage.colorSpace, 'oklab');
  assert.deepEqual(classStyle('bg-conic-180 from-danger to-accent').backgroundImage.from, 180);
  assert.deepEqual(classStyle('bg-[radial-gradient(circle_closest-side_at_left_top,#fff,#000_50%,#f00)]').backgroundImage, {
    kind: 'radial', shape: 'circle', size: 'closest-side', at: [0, 0], colorSpace: 'srgb',
    stops: [{ offset: 0, color: '#ffffffff' }, { offset: .5, color: '#000000ff' }, { offset: 1, color: '#ff0000ff' }],
  });
  const glow = classStyle('bg-[image:var(--glow)]').backgroundImage;
  assert.deepEqual([glow.shape, glow.at, glow.stops.map(stop => stop.offset)], ['circle', [.5, 0], [0, .7]]);
  const hero = classStyle('bg-(image:--gradient-hero)').backgroundImage;
  assert.deepEqual([hero.kind, hero.angle, hero.stops[1].offset], [undefined, 135, .8]);
  assert.deepEqual(classStyle('bg-[conic-gradient(from_90deg,#fff_0deg,#000_0.5turn)]').backgroundImage.stops.map(stop => stop.offset), [0, .5]);
  // Both set one background-image: the class written later wins.
  const mixed = 'bg-(image:--gradient-hero) bg-linear-45 from-danger to-accent';
  assert.equal(classStyle(mixed).backgroundImage.angle, 45);
  assert.equal(classStyle('bg-linear-45 bg-(image:--gradient-hero) from-danger to-accent').backgroundImage.angle, 135);
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
  assert.deepEqual(classStyle('rounded-br-md rounded-3xl'), { borderRadius: 24 }, 'a later shorthand clears the corner');
  assert.deepEqual(classStyle('rounded-3xl rounded-t-none'), { borderRadius: 24, borderTopLeftRadius: 0, borderTopRightRadius: 0 });
  assert.equal(classStyle('rounded-br-full').borderBottomRightRadius, 16384);
  const inline = node('column', { className: 'rounded-3xl rounded-br-md', style: { borderRadius: 8 } });
  assert.deepEqual([inline.radius, inline.radiusBottomRight], [8, 8], 'an inline shorthand resets class longhands');
  assert.throws(() => classStyle('[width:calc(infinity*1px)]'), /UI_CLASS/);
});

test('real Tailwind utilities lower to native dimensions, typography and colors', async () => {
  await use(['p-6', 'gap-2', 'text-sm', 'font-semibold', 'rounded-xl', 'bg-accent/80', 'opacity-50']);
  assert.deepEqual(classStyle('p-6 gap-2 text-sm font-semibold rounded-xl opacity-50'), {
    paddingTop: 24, paddingRight: 24, paddingBottom: 24, paddingLeft: 24, gap: 8, fontSize: 14, lineHeight: 1.25 / 0.875,
    fontWeight: 600, borderRadius: 12, opacity: .5,
  });
  assert.equal(node('text', { className: 'text-sm' }).lineHeight, 20);
  assert.equal(classStyle('bg-accent/80').backgroundColor, 'accent/80');
});

test('conflicts go to the class written later; variants keep Tailwind order and respond to viewport and appearance', async () => {
  await use(['gap-2', 'md:gap-4', 'bg-surface', 'dark:p-1', 'p-4', 'p-2', 'pl-1', 'md:p-8']);
  assert.equal(classStyle('p-4 p-2').paddingLeft, 8);
  assert.equal(classStyle('md:gap-4 gap-2', env('light', 900)).gap, 16, 'a later base class does not beat a breakpoint');
  assert.equal(classStyle('p-2 p-4').paddingLeft, 16);
  assert.deepEqual(classStyle('gap-2 md:gap-4 bg-surface', env('light', 767)), { gap: 8, backgroundColor: 'surface' });
  assert.deepEqual(classStyle('gap-2 md:gap-4 bg-surface dark:p-1', env('dark', 768)), { gap: 16, backgroundColor: 'surface', paddingTop: 4, paddingRight: 4, paddingBottom: 4, paddingLeft: 4 });
  assert.equal(classStyle('pl-1 md:p-8', env('light', 900)).paddingLeft, 32);
});

test('custom theme tokens and arbitrary values retain Tailwind semantics', async () => {
  await use(['bg-accent', 'rounded-xl', 'h-[50px]', 'text-[17px]'], '@theme { --color-accent: #007aff; --radius-xl: 16px; }');
  // A project theme cannot redefine a role: the palette, not the build, decides its colour.
  assert.deepEqual(classStyle('bg-accent rounded-xl h-[50px] text-[17px]'), { backgroundColor: 'accent', borderRadius: 16, height: 50, fontSize: 17 });
});

test('unsupported rendering and missing dynamic candidates fail visibly', async () => {
  const rules = await compileUtilities(new Set(['backdrop-invert', 'transition-[width]', 'skew-y-3', 'group-hover:underline', 'p-4']));
  const errors = Object.fromEntries(rules.filter(rule => rule.error).map(rule => [rule.name, rule.error]));
  assert.deepEqual(Object.keys(errors).sort(), ['backdrop-invert', 'group-hover:underline', 'skew-y-3', 'transition-[width]']);
  assert.ok(Object.values(errors).every(error => error.category === 'unsupported'));
  assert.match(errors['transition-[width]'].message, /animates layout/);
  configureClasses(rules);
  for (const name of ['backdrop-invert', 'transition-[width]', 'p-8']) assert.throws(() => classStyle(name), /UI_CLASS/);
  assert.throws(() => classStyle(false), /className must be a string/);
  assert.deepEqual(classStyle(undefined), {});
  assert.deepEqual(classStyle('p-4'), { paddingTop: 16, paddingRight: 16, paddingBottom: 16, paddingLeft: 16 });
});

test('stylesheets cannot silently discard browser selectors or imports', async () => {
  for (const css of [':root { color: red; }', '@import "tailwindcss";', '.card .title { padding: 12px; }'])
    await assert.rejects(compileUtilities(new Set(['p-4']), css), /UI_CSS/);
});

test('role classes keep their role in every appearance and refuse dark: and palette colours', async () => {
  await use(['bg-surface/80', 'text-muted', 'text-accent', 'dark:text-on-accent', 'text-white', 'bg-[#123456]'], '@theme inline { --color-surface: #ffffff; }');
  for (const theme of ['light', 'dark']) assert.deepEqual(classStyle('bg-surface/80 text-muted', { theme }), { backgroundColor: 'surface/80', color: 'text-muted' });
  assert.equal(classStyle('text-accent', { theme: 'dark' }).color, 'accent');
  assert.throws(() => classStyle('dark:text-on-accent'), /UI_CLASS: dark:text-on-accent: roles follow appearance; drop dark:/);
  for (const name of ['text-white', 'bg-[#123456]']) assert.throws(() => classStyle(name), /not a role; use a role class such as bg-surface/);
});

test('palette rules obey source order and diagnose invalid variables in either appearance', async () => {
  await use(['rounded-card'], `
    @theme inline { --radius-card: var(--round); }
    [data-theme="dark"] { --round: 2px; }
    :root { --round: 4px; }
    [data-theme="light"] { --round: 6px; }
  `);
  assert.equal(classStyle('rounded-card', { theme: 'dark' }).borderRadius, 4);
  assert.equal(classStyle('rounded-card', { theme: 'light' }).borderRadius, 6);
  for (const value of ['var(--missing)', 'var(--round)']) {
    await use(['rounded-card'], `@theme inline { --radius-card: var(--round); } :root { --round: 4px; } [data-theme="dark"] { --round: ${value}; }`);
    assert.throws(() => classStyle('rounded-card', { theme: 'light' }), /UI_CLASS.*(?:unresolved|cyclic)/);
  }
  for (const css of ['[data-theme="dark"] { color-scheme: light; }', '[data-theme="dark"] { --fill: black !important; }'])
    await assert.rejects(compileUtilities(new Set(['p-4']), css), /UI_CSS/);
  await assert.rejects(compileUtilities(new Set(['p-4']), '@theme { @keyframes 1spin { to { opacity: 0 } } }'), /UI_ANIMATION/);
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
  const texts = decode(describe(() => h('column', { className: 'text-sm' }, h(Label, {}, 'child'), h(listTag, { count: 1, row: () => h(Label, {}, 'row') })), {}))
    .filter(record => record.tag === 'text');
  assert.deepEqual(texts.map(text => text.props.lineHeight), [30, 30]);
  assert.equal(node('text', { className: 'text-[10px] tracking-widest' }).letterSpacing, 1);
  assert.equal(node('text', { className: 'text-[10px] tracking-tight' }).letterSpacing, -0.25);
  assert.deepEqual([classStyle('tracking-normal').letterSpacing, classStyle('tracking-[-1px]').letterSpacing], ['0em', -1]);
});

test('padding, borders and word breaking keep shorthand, axis, edge and variant semantics', async () => {
  await use(['p-2', 'px-6', 'py-3', 'pl-1', 'md:p-4', 'dark:px-3', '[padding:1px_2px_3px_4px]', 'border', 'border-4', 'border-t-2', 'border-x-8',
    'border-accent/50', 'border-current', 'border-dashed', 'border-none', 'break-all', 'break-normal', 'wrap-anywhere', 'h-auto', 'min-h-[42px]', 'max-h-[120px]']);
  const names = 'pl-1 px-6 p-2 py-3 md:p-4 dark:px-3';
  assert.deepEqual(classStyle(names, env('light', 600)), { paddingTop: 12, paddingRight: 8, paddingBottom: 12, paddingLeft: 8 });
  assert.deepEqual(classStyle(names, env('light', 800)), { paddingTop: 16, paddingRight: 16, paddingBottom: 16, paddingLeft: 16 });
  assert.deepEqual(classStyle(names, env('dark', 800)), { paddingTop: 16, paddingRight: 12, paddingBottom: 16, paddingLeft: 12 });
  assert.deepEqual(classStyle('[padding:1px_2px_3px_4px]'), { paddingTop: 1, paddingRight: 2, paddingBottom: 3, paddingLeft: 4 });
  const inline = node('column', { className: 'px-6 py-3', style: { paddingLeft: 7, padding: 5 } });
  assert.deepEqual([inline.paddingTop, inline.paddingRight, inline.paddingLeft], [5, 5, 7]);
  assert.deepEqual(classStyle('border'), { borderTopWidth: 1, borderRightWidth: 1, borderBottomWidth: 1, borderLeftWidth: 1 });
  assert.equal(classStyle('border-4 border-t-2').borderTopWidth, 2);
  assert.deepEqual(classStyle('border-x-8'), { borderLeftWidth: 8, borderRightWidth: 8 });
  assert.equal(classStyle('border-current').borderColor, 'currentColor');
  assert.equal(classStyle('border-accent/50').borderColor, 'accent/50');
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
  await use(['text-accent', 'placeholder:text-muted/30', 'md:placeholder:text-danger', 'placeholder:font-bold', 'placeholder:bg-danger']);
  const names = 'text-accent placeholder:text-muted/30 md:placeholder:text-danger';
  const light = classStyle(names, env()), dark = classStyle(names, env('dark'));
  assert.equal(light.placeholderColor, 'text-muted/30');
  assert.equal(dark.placeholderColor, 'text-muted/30');
  assert.equal(light.color, dark.color);
  assert.notEqual(classStyle(names, env('light', 900)).placeholderColor, light.placeholderColor);
  for (const name of ['placeholder:font-bold', 'placeholder:bg-danger']) assert.throws(() => classStyle(name), /placeholder supports only color/);
});

test('editor focus paint preserves cascade order without affecting base style', async () => {
  await use(['bg-surface', 'focus:bg-danger', 'md:focus:bg-accent', 'focus:shadow-lg', 'focus:placeholder:text-muted', 'focus:p-4']);
  const classes = 'bg-surface focus:bg-danger md:focus:bg-accent focus:shadow-lg';
  assert.deepEqual(classStyle(classes, env()), { backgroundColor: 'surface' });
  const focus = resolveClasses(classes, env()).focus;
  assert.ok(focus.boxShadow.length);
  assert.notEqual(focus.backgroundColor, resolveClasses(classes, env('light', 900)).focus.backgroundColor);
  assert.deepEqual(focus, resolveClasses(classes.split(' ').reverse().join(' '), env()).focus);
  assert.throws(() => classStyle('focus:placeholder:text-muted'), /UI_CLASS/);
  assert.throws(() => describe(() => h('input', { className: 'focus:p-4' }), {}), /focus: supports paint properties only/);
  assert.throws(() => describe(() => h('button', { className: 'focus:bg-danger' }, 'x'), {}), /focus: requires an editor/);
});

test('every Tailwind breakpoint matches, including the escaped 2xl', async () => {
  await use(['sm:p-1', 'md:p-2', 'lg:p-3', 'xl:p-4', '2xl:p-5']);
  const at = width => classStyle('sm:p-1 md:p-2 lg:p-3 xl:p-4 2xl:p-5', env('light', width)).paddingTop;
  assert.deepEqual([at(700), at(800), at(1100), at(1300), at(1600)], [4, 8, 12, 16, 20]);
});

test('relative variants, undrawn properties and no-op utilities fail with what to write instead', async () => {
  const rules = new Map((await compileUtilities(new Set(['*:p-2', '**:p-2', 'in-focus:p-2', 'group-hover:p-2', 'peer-checked:p-2',
    'cursor-pointer', 'pointer-events-none', 'outline-2', 'sepia', 'translate-z-4', 'scale-z-50', 'transform', 'transform-cpu',
    'drop-shadow-red-500', 'text-shadow-red-500', 'mask-radial-at-center', 'space-x-reverse', 'divide-y-reverse', 'select-auto'])))
    .map(rule => [rule.name, rule.error?.message]));
  const expect = {
    '*:p-2': /child variants .*each child/, '**:p-2': /descendant variants/, 'in-focus:p-2': /ancestor-state variants .*onFocus/,
    'group-hover:p-2': /group-\*\/peer-\*/, 'peer-checked:p-2': /group-\*\/peer-\*/, 'cursor-pointer': /no pointer cursor/,
    'pointer-events-none': /hit testing follows handlers/, 'outline-2': /ring-\*/, sepia: /sets nothing/,
    'translate-z-4': /3D transforms/, 'scale-z-50': /3D transforms/, transform: /apply without it/, 'transform-cpu': /apply without it/,
    'drop-shadow-red-500': /shadow-<color>/, 'text-shadow-red-500': /text shadows/, 'mask-radial-at-center': /radial and conic masks/,
    'space-x-reverse': /gap-\*/, 'divide-y-reverse': /gap-\*/, 'select-auto': /select-text or select-none/,
  };
  for (const [name, pattern] of Object.entries(expect)) {
    assert.match(rules.get(name) ?? '', pattern, name);
    assert.doesNotMatch(rules.get(name), /transitions/, name);
  }
});

test('justify-self-start and -end place grid items like justify-items', async () => {
  await use(['justify-self-start', 'justify-self-end', 'justify-self-center']);
  assert.deepEqual(['start', 'end', 'center'].map(edge => classStyle(`justify-self-${edge}`).justifySelf), ['start', 'end', 'center']);
});

test('elevation shadows take the theme shadow ink; literal tints and transparent layers stay', async () => {
  await use(['shadow-md', 'shadow-xs']);
  assert.deepEqual(classStyle('shadow-md').boxShadow.map(layer => layer.color), ['shadow/10', 'shadow/10']);
  assert.deepEqual(classStyle('shadow-xs').boxShadow.map(layer => layer.color), ['shadow/5']);
});

// A web app's theme layer ported to View tokens: its classes are `@utility` rules.
test('theme @utility classes lower like utilities, with background and border shorthands', async () => {
  const theme = `@theme inline { --color-card: var(--color-surface); --color-primary: var(--color-accent); }
:root { --card-shadow: 0 1px 2px rgb(0 0 0 / 6%); }
[data-theme='dark'] { --card-shadow: none; }
@utility hero { background: radial-gradient(120% 90% at 50% 0%, color-mix(in srgb, var(--color-primary) 10%, transparent) 0%, transparent 58%), var(--color-card); }
@utility brand { background-image: linear-gradient(135deg, var(--color-primary) 0%, var(--color-chart-1) 100%); }
@utility ring-soft { border: 1px solid color-mix(in srgb, var(--color-primary) 30%, transparent); }
@utility numeric { font-variant-numeric: tabular-nums; }
@utility density-compact {}`;
  const rules = await compileUtilities(new Set(['hero', 'brand', 'ring-soft', 'numeric', 'density-compact', 'group', 'shadow-[var(--card-shadow)]', 'unknown-marker']), theme);
  const rule = (name, when) => rules.find(r => r.name === name && (!when || r.when?.theme === when));
  assert.equal(rule('hero').style.backgroundColor, 'surface');
  assert.deepEqual(rule('hero').gradient.fixed, { kind: 'radial', at: [0.5, 0], radii: [1.2, 0.9], colorSpace: 'srgb', stops: [{ offset: 0, color: 'accent/10' }, { offset: 0.58, color: '#00000000' }] });
  assert.deepEqual(rule('brand').gradient.fixed.stops.map(stop => stop.color), ['accent', 'chart-1']);
  assert.deepEqual([rule('ring-soft').style.borderTopWidth, rule('ring-soft').style.borderStyle, rule('ring-soft').style.borderColor], [1, 'solid', 'accent/30']);
  assert.equal(rule('numeric').numeric.fixed, 'tabular-nums');
  for (const marker of ['density-compact', 'group']) assert.deepEqual(rule(marker).style, {});
  assert.equal(rule('unknown-marker'), undefined);
  assert.deepEqual(rule('shadow-[var(--card-shadow)]', 'dark').shadow.layers, []);
  await assert.rejects(compileUtilities(new Set(['x']), '@utility x { &:hover { color: red; } }'), /UI_CSS: @utility x takes declarations only/);
});

// Tailwind themes write classes as plain rules or in `@layer components`; each is that `@utility`.
test('plain .class rules and @layer components classes are theme utilities', async () => {
  const theme = `@theme inline { --color-primary: var(--color-accent); }
.brand { background-image: linear-gradient(135deg, var(--color-primary) 0%, var(--color-chart-1) 100%); }
@layer components { .numeric { font-variant-numeric: tabular-nums; } }`;
  const rules = await compileUtilities(new Set(['brand', 'numeric']), theme);
  assert.deepEqual(rules.find(r => r.name === 'brand').gradient.fixed.stops.map(stop => stop.color), ['accent', 'chart-1']);
  assert.equal(rules.find(r => r.name === 'numeric').numeric.fixed, 'tabular-nums');
  await assert.rejects(compileUtilities(new Set(['x']), '.card > .title { color: red; }'), /UI_CSS: expected @theme, :root or \[data-theme\] tokens, @utility, or a plain \.class rule/);
});

test('safe-area env() is zero inside a page; max(), min() and system font stacks fold', async () => {
  const rules = await compileUtilities(new Set(['pb-[max(1.5rem,env(safe-area-inset-bottom))]', 'w-[min(10px,2rem)]', 'w-[max(10px,50%)]', 'font-brand', 'font-ui', 'sr-only']),
    `@theme { --font-brand: 'Songti SC', ui-serif, serif; --font-ui: system-ui, -apple-system, 'PingFang SC', sans-serif; }`);
  const rule = name => rules.find(r => r.name === name);
  assert.equal(rule('pb-[max(1.5rem,env(safe-area-inset-bottom))]').style.paddingBottom, 24);
  assert.equal(rule('w-[min(10px,2rem)]').style.width, 10);
  assert.match(rule('w-[max(10px,50%)]').error.message, /unsupported calculation/);
  assert.deepEqual([rule('font-brand').style.fontFamily, rule('font-ui').style.fontFamily], ['serif', 'sans-serif']);
  // Visually hidden, still read: opacity leaves semantics.
  assert.deepEqual([rule('sr-only').style.position, rule('sr-only').style.opacity], ['absolute', 0]);
});

test('group-active: paints while the enclosing control is pressed', async () => {
  await use(['group', 'group-active:scale-95', 'group-active:opacity-50']);
  const button = rendered(() => h('button', { className: 'group', onClick() {} }, h('view', { className: 'group-active:scale-95 group-active:opacity-50' })));
  const child = button.children[0].props;
  assert.ok(child.groupPressedStyle, 'the child carries its group-pressed paint');
  assert.equal(button.props.pressedStyle, undefined);
});

test('dir="auto" takes the direction of the content\'s first strong character', () => {
  const root = rendered(() => h('column', {}, h('text', { dir: 'auto' }, '42 שלום world'), h('text', { dir: 'auto' }, '123 hello'),
    h('row', { dir: 'auto' }, h('text', {}, 'مرحبا')), h('text', { dir: 'auto' }, '123')));
  assert.deepEqual(root.children.map(child => child.props.direction), ['rtl', 'ltr', 'rtl', undefined]);
  assert.throws(() => rendered(() => h('text', { dir: 'up' }, 'x')), /UI_PROP: dir must be "ltr", "rtl" or "auto"/);
});

// cva components merge the caller's className after their own, as tailwind-merge does.
test('a caller class overrides a component class of the same property; variants stay separate', async () => {
  await use(['h-10', 'px-4', 'rounded-md', 'bg-accent', 'text-on-accent', 'rounded-full', 'px-2', 'bg-danger', 'active:bg-accent', 'p-2', 'p-[3px]', 'md:px-6']);
  const component = 'h-10 px-4 rounded-md bg-accent text-on-accent active:bg-accent md:px-6';
  const merged = resolveClasses(`${component} rounded-full px-2 bg-danger`, env('light', 400));
  assert.deepEqual([merged.base.borderRadius, merged.base.paddingLeft, merged.base.backgroundColor, merged.base.color], [16384, 8, 'danger', 'on-accent']);
  assert.equal(merged.active.backgroundColor, 'accent', 'the pressed variant is not overridden by a base class');
  assert.equal(classStyle(`${component} px-2`, env('light', 900)).paddingLeft, 24, 'nor is a breakpoint');
  assert.equal(classStyle('p-2 p-[3px]').paddingTop, 3, 'arbitrary values take part');
  assert.equal(classStyle('p-[3px] p-2').paddingTop, 8);
});

test('a gradient sweep: a sized background layer and a background-position keyframe', async () => {
  const theme = `@theme inline { @keyframes sweep { from { background-position: 100% 0; } to { background-position: -150% 0; } } }
@utility sweep { background: linear-gradient(90deg, var(--color-muted) 40%, var(--color-text) 50%, var(--color-muted) 60%) 0 0 / 250% 100%; animation: sweep 1.8s linear infinite; }`;
  const [rule] = await compileUtilities(new Set(['sweep']), theme);
  assert.deepEqual([rule.gradient.fixed.layerSize, rule.gradient.fixed.layerPosition], [[2.5, 1], [0, 0]]);
  assert.deepEqual(rule.style.animation.backgroundPosition, { x: [{ offset: 0, value: 1 }, { offset: 1, value: -1.5 }], y: [{ offset: 0, value: 0 }, { offset: 1, value: 0 }] });
  await assert.rejects(async () => { const [bad] = await compileUtilities(new Set(['half']), `@theme inline { @keyframes half { 50% { background-position: 0 0; } } }
@utility half { animation: half 1s; }`); if (bad.error) throw Error(bad.error.message); }, /from and to/);
});
