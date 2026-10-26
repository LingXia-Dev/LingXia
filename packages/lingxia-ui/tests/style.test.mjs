import { test, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { h, describe, configureClasses } from '../src/internal.js';
import { rendered } from './support/wire.mjs';

afterEach(() => configureClasses([]));
const style = (value, tag = 'column') => rendered(() => h(tag, { style: value })).props;
const rejects = (value, pattern, tag = 'column') => assert.throws(() => describe(() => h(tag, { style: value }), {}), pattern);

test('dimensions lower lengths, percentages, viewport units and sizing keywords', () => {
  assert.equal(style({ width: 100 }).width, 100);
  for (const name of ['width', 'height', 'minWidth', 'minHeight', 'maxWidth', 'maxHeight']) {
    assert.equal(style({ [name]: '72%' })[`${name}Percent`], 72);
    for (const value of [-1, NaN, Infinity, null, '-1%', '2px', '1e2%', '16385%', '50 %', '10%junk'])
      rejects({ [name]: value }, /UI_STYLE_VALUE/);
  }
  assert.equal(style({ height: 'auto' }).heightAuto, 1);
  assert.equal(style({ width: 'fit-content' }).widthKeyword, 'fit-content');
  rejects({ minWidth: 'fit-content' }, /UI_STYLE_VALUE/);
  const viewport = rendered(() => h('column', { style: { width: '100vw', height: '50vh' } }), { env: { viewport: { width: 360, height: 800 } } }).props;
  assert.deepEqual([viewport.width, viewport.height], [360, 400]);
  rejects({ widthPercent: 50 }, /UI_STYLE: unknown style property widthPercent/);
});

test('flex basis encodes auto, logical lengths and percentages without mixing them', () => {
  for (const value of ['auto', '0%', '50%', 0, 48]) assert.equal(style({ flexBasis: value }).flexBasis, value);
  for (const value of [-1, Infinity, NaN, 16385, '-1%', '1e2%', 'auto ', '10px', 'content', null]) rejects({ flexBasis: value }, /UI_STYLE_VALUE/);
});

test('CSS names lower to the native box: colors, radii, flex factors and alignment', () => {
  const props = style({ backgroundColor: '#ffffff', borderRadius: 8, borderTopLeftRadius: 2, flexGrow: 1, flexShrink: 0,
    alignItems: 'flex-start', justifyContent: 'space-between', alignSelf: 'flex-end' }, 'row');
  assert.deepEqual([props.background, props.radius, props.radiusTopLeft, props.radiusTopRight, props.grow, props.shrink, props.align, props.justify, props.alignSelf],
    ['#ffffff', 8, 2, 8, 1, 0, 'start', 'between', 'end']);
  assert.deepEqual([style({ justifyContent: 'space-around' }).justify, style({ justifyContent: 'space-evenly' }).justify], ['around', 'evenly']);
  for (const value of [{ alignItems: 'space-between' }, { justifyContent: 'stretch' }, { alignSelf: 'auto' }, { alignItems: 1 }]) rejects(value, /UI_STYLE_VALUE/);
  for (const value of [{ alignItems: 'center' }, { justifyContent: 'flex-end' }])
    assert.throws(() => describe(() => h('list', { count: 0, row: () => h('text', {}, 'row'), style: value }), {}), /is not supported on <list>/);
});

test('longhands win over shorthands regardless of declaration order', () => {
  const props = style({ paddingTop: 2, padding: 8, marginLeft: 'auto', margin: 4, borderBottomWidth: 3, borderWidth: 1 });
  assert.deepEqual([props.paddingTop, props.paddingRight, props.marginLeft, props.marginTop, props.borderBottomWidth, props.borderTopWidth],
    [2, 8, 'auto', 4, 3, 1]);
  for (const name of ['padding', 'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft'])
    for (const value of [-1, 16385, Infinity, NaN, '12', null, true]) rejects({ [name]: value }, /UI_STYLE_VALUE/);
  rejects({ margin: 'x' }, /UI_STYLE_VALUE/);
  assert.equal(style({ marginTop: -8 }).marginTop, -8);
});

test('positioning, layering and clipping are native box properties', () => {
  const props = style({ position: 'absolute', inset: 0, right: '10%', zIndex: 3, overflow: 'hidden' });
  assert.deepEqual([props.position, props.insetTop, props.insetRight, props.insetLeft, props.zIndex, props.overflow], ['absolute', 0, '10%', 0, 3, 'hidden']);
  assert.equal(style({ position: 'fixed' }).position, 'fixed');
  for (const value of [{ position: 'sticky' }, { zIndex: 1.5 }, { zIndex: 10001 }, { top: 'auto' }, { overflow: 'scroll' }]) rejects(value, /UI_STYLE_VALUE/);
});

test('outer shadows, gradients and backdrop filters are bounded composite values', () => {
  assert.deepEqual(JSON.parse(style({ boxShadow: [{ offsetY: 2, blur: 4 }] }).boxShadow), [{ offsetX: 0, offsetY: 2, blur: 4, spread: 0, color: 'currentColor' }]);
  for (const value of [null, {}, 'red', [{ inset: true }], [{ blur: -1 }], [{ blur: 257 }], [{ spread: 1025 }], [{ offsetX: null }], [{ color: 'red' }], Array(9).fill({})])
    rejects({ boxShadow: value }, /UI_SHADOW/);
  assert.equal(style({ boxShadow: 'none' }).boxShadow, 'none');
  const gradient = { direction: 'right', stops: [{ offset: 0, color: '#ff0000' }, { offset: 1, color: '#0000ff80' }] };
  assert.deepEqual(JSON.parse(style({ backgroundImage: gradient }).backgroundGradient), { ...gradient, colorSpace: 'srgb' });
  for (const value of [null, 'red', {}, { ...gradient, angle: 30 }, { ...gradient, unknown: true }, { ...gradient, colorSpace: 'hsl' }, { ...gradient, stops: [] }])
    rejects({ backgroundImage: value }, /UI_GRADIENT/);
  const stops = [{ offset: 0, color: '#ff0000' }, { offset: 1, color: '#0000ff' }];
  for (const value of [{ kind: 'radial', shape: 'circle', size: 'closest-side', at: [0.25, 0.75], stops }, { kind: 'conic', from: 90, at: [0.5, 0.5], stops }])
    assert.deepEqual(JSON.parse(style({ backgroundImage: value }).backgroundGradient), { ...value, colorSpace: 'srgb' });
  for (const value of [{ kind: 'radial', angle: 30, stops }, { kind: 'conic', shape: 'circle', stops }, { kind: 'linear', at: [0, 0], stops },
    { kind: 'radial', stops: [{ offset: -0.1, color: '#ff0000' }, stops[1]] }, { kind: 'conic', stops: [stops[0], { offset: 1.5, color: '#0000ff' }] }, { kind: 'repeating', stops }])
    rejects({ backgroundImage: value }, /UI_GRADIENT/);
  const filter = style({ backdropFilter: 'blur(12px) saturate(150%)' });
  assert.deepEqual([filter.backdropBlur, filter.backdropSaturation], [12, 1.5]);
  for (const value of ['blur(65px)', 'saturate(17)', 'blur(4)', 'invert(1)', 4, null]) rejects({ backdropFilter: value }, /UI_STYLE_VALUE/);
});

test('static transform style keys lower to one bounded wire transform', () => {
  assert.deepEqual(JSON.parse(style({ translate: 4, rotate: 30, scale: 2, transformOrigin: [0, 1] }).transform),
    { translate: [4, 0], rotate: 30, scale: [2, 2], origin: [0, 1] });
  assert.deepEqual(JSON.parse(style({ translate: ['-50%', 3] }).transform), { translate: ['-50%', 3] });
  for (const value of [{ translate: '2000%' }, { rotate: 'x' }, { scale: [1, 1, 1] }, { scale: 1e4 }, { transformOrigin: 0.5 }, { translate: [1, 'a'] }])
    rejects(value, /UI_STYLE_VALUE/);
});

test('typography: CSS line-height multipliers inherit and recompute per font size', () => {
  const root = rendered(() => h('column', { style: { fontSize: 10, lineHeight: 1.5 } }, h('text', { style: { fontSize: 20 } }, 'big'), h('text', {}, 'small')));
  assert.equal(root.props.lineHeight, 15);
  assert.deepEqual(root.children.map(child => child.props.lineHeight), [30, 15]);
  assert.equal(style({ lineHeight: '24px' }, 'text').lineHeight, 24);
  assert.equal(style({ fontSize: 20, letterSpacing: '0.1em' }, 'text').letterSpacing, 2);
  assert.equal(style({ fontFamily: 'monospace', fontWeight: 650 }, 'text').fontFamily, 'mono');
  for (const value of [{ fontWeight: 0 }, { fontWeight: 1001 }, { fontWeight: 'bold' }, { lineHeight: 0 }, { lineHeight: Infinity }, { lineHeight: '24' },
    { fontFamily: 'Arial' }, { fontFamily: 'mono' }]) rejects(value, /UI_STYLE_VALUE/, 'text');
  const text = style({ textTransform: 'uppercase', whiteSpace: 'nowrap', textOverflow: 'ellipsis', textDecorationLine: 'underline' }, 'text');
  assert.deepEqual([text.textTransform, text.whiteSpace, text.textOverflow, text.textDecorationLine], ['uppercase', 'nowrap', 'ellipsis', 'underline']);
});

test('opacity, placeholder and scroll axes accept only their element and range', () => {
  for (const opacity of [0, 0.25, 1]) assert.equal(style({ opacity }).opacity, opacity);
  for (const opacity of [-0.01, 1.01, NaN, Infinity, '0.5', null, true]) rejects({ opacity }, /UI_STYLE_VALUE/);
  assert.equal(style({ placeholderColor: '#12345680' }, 'input').placeholderColor, '#12345680');
  for (const value of [null, 1, true, {}]) rejects({ placeholderColor: value }, /UI_STYLE_VALUE/, 'input');
  rejects({ placeholderColor: '#123456' }, /not supported on <column>/);
  assert.equal(style({ overflowX: 'auto' }, 'scroll').scrollAxis, 'horizontal');
  rejects({ overflowX: 'auto' }, /UI_STYLE: overflowX is not supported on <column>/);
});

test('every style property is checked against the element that renders it', () => {
  for (const [tag, name, value] of [['text', 'alignItems', 'center'], ['text', 'gap', 4], ['image', 'color', '#000000'], ['spacer', 'padding', 4],
    ['plot', 'fontSize', 12], ['row', 'lineClamp', 2], ['button', 'flexDirection', 'row'], ['slider', 'fontSize', 12]])
    rejects({ [name]: value }, new RegExp(`UI_STYLE: ${name} is not supported on <${tag}>`), tag);
  rejects({ unknownThing: 1 }, /UI_STYLE: unknown style property unknownThing/);
});

const rules = list => configureClasses(list.map(([name, style, when = {}]) => ({ name, when, style })));

test('active and hover variants become paint deltas; hover only for fine pointers', () => {
  rules([['bg-white', { backgroundColor: '#ffffffff' }], ['active:bg-gray', { backgroundColor: '#eeeeeeff' }, { state: 'active' }],
    ['hover:bg-blue', { backgroundColor: '#0000ffff' }, { state: 'hover' }], ['active:p-2', { paddingTop: 8 }, { state: 'active' }]]);
  const touch = rendered(() => h('button', { className: 'bg-white active:bg-gray hover:bg-blue' }, 'Go'), { env: { pointer: 'coarse' } });
  assert.deepEqual(touch.props.pressedStyle, { background: '#eeeeeeff' });
  assert.equal(touch.props.hoverStyle, undefined);
  const mouse = rendered(() => h('button', { className: 'bg-white hover:bg-blue' }, 'Go'), { env: { pointer: 'fine' } });
  assert.deepEqual(mouse.props.hoverStyle, { background: '#0000ffff' });
  assert.throws(() => describe(() => h('column', { className: 'active:bg-gray' }), {}), /UI_STYLE: active: requires a control, not <column>/);
  assert.throws(() => describe(() => h('button', { className: 'active:p-2' }, 'x'), {}), /active: supports paint properties only, not paddingTop/);
});

test('disabled variants replace the cascade only while disabled', () => {
  rules([['opacity-100', { opacity: 1 }], ['disabled:opacity-50', { opacity: 0.5 }, { state: 'disabled' }]]);
  const render = disabled => () => h('button', { className: 'opacity-100 disabled:opacity-50', disabled }, 'Save');
  assert.equal(rendered(render(false)).props.opacity, 1);
  assert.equal(rendered(render(true)).props.opacity, 0.5);
});

test('space and divide rules style every child but the last, below the child own classes', () => {
  rules([['space-y-4', { marginTop: 0, marginBottom: 16 }, { children: true }], ['divide-y', { borderTopWidth: 0, borderBottomWidth: 1 }, { children: true }],
    ['mb-1', { marginBottom: 4 }]]);
  const root = rendered(() => h('column', { className: 'space-y-4 divide-y' }, h('text', {}, 'a'), h('text', { className: 'mb-1' }, 'b'), h('text', {}, 'c')));
  assert.deepEqual(root.children.map(child => child.props.marginBottom), [16, 4, undefined]);
  assert.deepEqual(root.children.map(child => child.props.borderBottomWidth), [1, 1, undefined]);
});
