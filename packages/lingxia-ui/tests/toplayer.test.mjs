import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, dispatch, useLocal, reset } from '../src/internal.js';
import { escapeOverlay } from '../src/runtime/overlay.js';
import { hasLocalUpdates } from '../src/runtime/hooks.js';
import { takeActionErrors } from '../src/runtime/actions.js';
import { compileUtilities } from '../compiler/tailwind.mjs';
import { configureClasses } from '../src/style/classes.js';
import { decode } from './support/wire.mjs';

const render = (view, env = {}) => { const out = decode(prepare(view, { env })); commit(); return out; };
const byId = (nodes, id) => nodes.find(node => node.testId === id);

test('an open dialog is a top-layer box with a backdrop, a modal role and auto margins', () => {
  reset();
  let close = 0;
  const view = () => h('page', {}, h('row', {},
    h('dialog', { open: true, onClose: () => close++, closedby: 'any', 'aria-label': 'Delete?', 'data-testid': 'd', style: { marginTop: 'auto' } }, h('text', {}, 'Sure?'))));
  const nodes = render(view, { keyboard: { height: 300 } });
  const panel = byId(nodes, 'd'), backdrop = byId(nodes, 'd-backdrop');
  const layer = nodes.find(node => node.children.includes(panel));
  assert.equal(layer.props.topLayer, 1);
  assert.equal(layer.props.position, 'absolute');
  assert.equal(layer.props.insetBottom, 300, 'it rises with the keyboard');
  assert.equal(panel.props.role, 'dialog');
  assert.equal(panel.props.accessibilityLabel, 'Delete?');
  assert.equal(panel.props.marginBottom, 'auto');
  assert.equal(backdrop.props.accessibilityHidden, 1);
  dispatch(backdrop.props.onClick);
  assert.equal(close, 1, 'closedby="any" closes on a backdrop tap');
  assert.equal(escapeOverlay(), true);
  assert.equal(close, 2, 'Escape is a close request');
});

test('a closed dialog renders nothing; closedby="none" ignores Escape and the backdrop', () => {
  reset();
  let close = 0;
  const shown = { value: false };
  const view = () => h('page', {}, h('dialog', { open: shown.value, onClose: () => close++, closedby: 'none', 'aria-label': 'Busy', 'data-testid': 'd' }));
  assert.equal(byId(render(view), 'd'), undefined);
  shown.value = true;
  const nodes = render(view);
  dispatch(byId(nodes, 'd-backdrop').props.onClick);
  assert.equal(escapeOverlay(), false);
  assert.equal(close, 0);
  assert.throws(() => render(() => h('page', {}, h('dialog', { open: true }))), /UI_A11Y/);
});

test('popoverTarget toggles an uncontrolled popover anchored to its invoker; light dismiss closes it', () => {
  reset();
  let toggled = [];
  const view = () => h('page', {},
    h('button', { popoverTarget: 'menu', 'data-testid': 'more', 'aria-label': 'More' }),
    h('view', { id: 'menu', popover: true, role: 'menu', onToggle: open => toggled.push(open), 'data-testid': 'menu' },
      h('button', { role: 'menuitem', 'data-testid': 'rename', onClick: () => toggled.push('rename') }, 'Rename')));
  let nodes = render(view);
  assert.equal(byId(nodes, 'menu'), undefined);
  assert.equal(byId(nodes, 'more').props.stateExpanded, 0);
  dispatch(byId(nodes, 'more').props.onClick);
  assert.equal(hasLocalUpdates(), true, 'an uncontrolled toggle re-renders');
  nodes = render(view);
  const menu = byId(nodes, 'menu');
  assert.equal(menu.props.role, 'menu');
  assert.equal(menu.props.anchorTo, '--lx-popover-menu');
  assert.equal(byId(nodes, 'more').props.anchorName, '--lx-popover-menu');
  assert.equal(menu.props.position, 'absolute');
  assert.equal(byId(nodes, 'more').props.stateExpanded, 1);
  dispatch(byId(nodes, 'rename').props.onClick);
  assert.deepEqual(toggled, [true, 'rename', false], 'a menuitem closes its menu after its click');
  nodes = render(view);
  assert.equal(byId(nodes, 'menu'), undefined);
  dispatch(byId(nodes, 'more').props.onClick);
  nodes = render(view);
  dispatch(byId(nodes, 'menu-backdrop').props.onClick);
  assert.equal(byId(render(view), 'menu'), undefined, 'a tap outside closes an auto popover');
});

test('a controlled popover follows open; opening one auto popover closes the others', () => {
  reset();
  const calls = [];
  const view = () => {
    const a = useLocal('a', false), b = useLocal('b', false);
    return h('page', {},
      h('button', { popoverTarget: 'a', 'data-testid': 'open-a' }, 'A'),
      h('button', { popoverTarget: 'b', 'data-testid': 'open-b' }, 'B'),
      h('view', { id: 'a', popover: 'auto', open: a.value, onToggle: next => { calls.push(['a', next]); a.set(next); }, 'data-testid': 'a' }),
      h('view', { id: 'b', popover: 'manual', open: b.value, onToggle: next => { calls.push(['b', next]); b.set(next); }, 'data-testid': 'b' }));
  };
  let nodes = render(view);
  dispatch(byId(nodes, 'open-a').props.onClick);
  nodes = render(view);
  assert.ok(byId(nodes, 'a'));
  dispatch(byId(nodes, 'open-b').props.onClick);
  nodes = render(view);
  assert.ok(byId(nodes, 'b'));
  assert.ok(byId(nodes, 'a'), 'a manual popover does not close auto ones');
  assert.equal(byId(nodes, 'b-backdrop'), undefined, 'a manual popover has no light dismiss');
  assert.equal(escapeOverlay(), true, 'Escape skips the manual popover and closes the auto one');
  assert.deepEqual(calls, [['a', true], ['b', true], ['a', false]]);
});

test('popover ids are unique, invokers name one, and past the budget an overlay stays closed', () => {
  reset();
  assert.throws(() => render(() => h('page', {}, h('button', { popoverTarget: 'x' }, 'X'))), /UI_ID: popoverTarget "x"/);
  assert.throws(() => render(() => h('page', {}, h('view', { id: 'x', popover: true }), h('view', { id: 'x', popover: true }))), /UI_ID/);
  assert.throws(() => render(() => h('page', {}, h('view', { open: true }))), /add popover/);
  assert.throws(() => render(() => h('page', {}, h('button', { role: 'menuitem' }, 'X'))), /inside a popover/);
  reset();
  const many = () => h('page', {}, ...Array.from({ length: 17 }, (_, i) => h('dialog', { key: `d${i}`, open: true, 'aria-label': `d${i}`, 'data-testid': `d${i}` })));
  const nodes = render(many);
  assert.equal(nodes.filter(node => node.props.role === 'dialog').length, 16);
  assert.match(takeActionErrors().join(), /UI_BUDGET/);
});

test('backdrop: styles a dialog backdrop and nothing else', async () => {
  reset();
  configureClasses(await compileUtilities(new Set(['backdrop:bg-text/40', 'bg-surface', '[position-area:top_span-inline-start]'])));
  const menu = render(() => h('page', {}, h('view', { id: 'm', popover: true, open: true, 'data-testid': 'm', className: '[position-area:top_span-inline-start]' })));
  assert.equal(byId(menu, 'm').props.anchorArea, 'top span-inline-start');
  const nodes = render(() => h('page', {}, h('dialog', { open: true, 'aria-label': 'x', 'data-testid': 'd', className: 'bg-surface backdrop:bg-text/40' })));
  assert.notEqual(byId(nodes, 'd-backdrop').props.backgroundColor, 'scrim');
  assert.throws(() => render(() => h('page', {}, h('view', { className: 'backdrop:bg-text/40' }))), /UI_STYLE: backdrop:/);
  configureClasses([]);
});
