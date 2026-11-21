import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { compileUtilities } from '../compiler/tailwind.mjs';
import { configureClasses } from '../src/style/classes.js';
import { h, describe } from '../src/internal.js';
import { rendered, byTestId } from './support/wire.mjs';

// A callout whose caret rides inside it: one caret per side, each hidden unless it faces the anchor.
const down = 'side-top:opacity-100 opacity-0', up = 'side-bottom:opacity-100 opacity-0';
const start = 'side-start:bg-accent-solid', end = 'side-end:bg-accent-solid';
before(async () => configureClasses(await compileUtilities(new Set(
  ['rounded-lg bg-surface-raised side-bottom:shadow-md', down, up, start, end].flatMap(names => names.split(' '))))));
after(() => configureClasses([]));

const popover = (className, children = () => []) => () => h('page', {}, h('view', { id: 'a' }),
  h('view', { id: 'tip', popover: 'manual', open: true, anchor: 'a', 'data-testid': 'tip', className }, ...children()));

test('side-* variants lower to paint-only state styles the core lays on by the side the popover ended on', () => {
  const page = rendered(popover('rounded-lg bg-surface-raised side-bottom:shadow-md', () => [h('view', { 'data-testid': 'down', className: down }),
    h('view', { 'data-testid': 'up', className: up }), h('view', { 'data-testid': 's', className: start }), h('view', { 'data-testid': 'e', className: end })]));
  const props = id => byTestId(page, id).props;
  assert.ok(props('tip').sideBottomStyle, 'the popover itself takes one');
  assert.equal(props('tip').sideTopStyle, undefined);
  assert.deepEqual([props('down').opacity, props('down').sideTopStyle?.opacity], [0, 1]);
  assert.deepEqual([props('up').opacity, props('up').sideBottomStyle?.opacity], [0, 1]);
  assert.ok(props('s').sideStartStyle && !props('s').sideEndStyle);
  assert.ok(props('e').sideEndStyle && !props('e').sideStartStyle);
});

test('side-* variants refuse layout, as checked: does', async () => {
  configureClasses(await compileUtilities(new Set(['side-top:mb-2'])));
  try {
    assert.throws(() => describe(popover('side-top:mb-2'), {}), /UI_STYLE: side-top: supports paint properties only, not marginBottom/);
  } finally { configureClasses([]); }
});

test('anchor(center) is an inset value; other anchor() forms are refused at compile time', async () => {
  const rules = await compileUtilities(new Set(['start-[anchor(center)]', 'top-[anchor(center)]', 'left-[anchor(start)]']));
  const rule = name => rules.find(entry => entry.name === name);
  assert.deepEqual(rule('start-[anchor(center)]').style, { insetInlineStart: 'anchor(center)' });
  assert.deepEqual(rule('top-[anchor(center)]').style, { top: 'anchor(center)' });
  assert.match(rule('left-[anchor(start)]').error.message, /only anchor\(center\) is supported/);
  const caret = byTestId(rendered(() => h('page', {}, h('view', { id: 'a' }), h('view', { id: 'tip', popover: 'manual', open: true, anchor: 'a' },
    h('view', { 'data-testid': 'caret', style: { position: 'absolute', left: 'anchor(center)', top: -6 } })))), 'caret').props;
  assert.equal(caret.insetLeft, 'anchor(center)');
});
