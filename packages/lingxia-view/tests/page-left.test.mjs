import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, dispatch, reset } from '../src/internal.js';
import { publishHost } from '../src/host.js';
import { viewCall, takeOverlayReplies, hasOverlay } from '../src/runtime/overlay.js';
import { openPicker, hasPicker, takePickerResults } from '../src/runtime/picker.js';
import { decode } from './support/wire.mjs';

const render = view => { const out = decode(prepare(view, {})); commit(); return out; };
const byId = (nodes, id) => nodes.find(node => node.testId === id);

test('leaving the page closes its picker, light-dismiss popovers, hints and action sheets, and keeps dialogs, manual popovers and toasts', () => {
  reset(); publishHost(() => h('view', {}));
  const toggled = [];
  const hint = { open: true };
  let dialogClosed = 0;
  const view = () => h('page', {},
    h('button', { popoverTarget: 'menu', 'data-testid': 'more', 'aria-label': 'More' }),
    h('view', { id: 'menu', popover: true, role: 'menu', onToggle: open => toggled.push(`menu ${open}`), 'data-testid': 'menu' },
      h('button', { role: 'menuitem', popoverTarget: 'sub', 'data-testid': 'share' }, 'Share')),
    h('view', { id: 'sub', popover: true, role: 'menu', onToggle: open => toggled.push(`sub ${open}`), 'data-testid': 'sub' }, h('text', {}, 'Mail')),
    // Controlled: it closes through its onToggle, as a press elsewhere closes it.
    h('view', { id: 'tip', popover: 'hint', role: 'tooltip', open: hint.open, onToggle: open => { hint.open = open; toggled.push(`tip ${open}`); }, 'data-testid': 'tip' }, h('text', {}, 'Tip')),
    h('view', { id: 'note', popover: 'manual', open: true, 'data-testid': 'note' }, h('text', {}, 'Saved')),
    h('dialog', { open: true, onClose: () => dialogClosed++, closedby: 'any', 'aria-label': 'Delete?', 'data-testid': 'confirm' }, h('text', {}, 'Sure?')));
  let nodes = render(view);
  dispatch(byId(nodes, 'more').props.onClick);
  nodes = render(view);
  dispatch(byId(nodes, 'share').props.onClick);
  // Opening the menu closed the hint; it shows again over it, as a hovered trigger's would.
  hint.open = true;
  nodes = render(view);
  assert.ok(['menu', 'sub', 'tip', 'note', 'confirm'].every(id => byId(nodes, id)), 'all open before the page is left');
  assert.equal(openPicker({ identity: '/i:0:0', kind: 'select', options: [{ value: 'a', label: 'A' }], selected: 0 }), true);
  viewCall({ id: 7, method: 'ui.showActionSheet', params: { itemList: ['Copy'] } });
  viewCall({ id: 8, method: 'ui.showToast', params: { title: 'Done', duration: 0 } });
  toggled.length = 0;

  assert.equal(globalThis.__lingxiaPageLeft(), true);
  assert.equal(hasPicker(), false);
  assert.equal(takePickerResults(), '[]', 'the select keeps its value: no choice is reported');
  assert.deepEqual(toggled, ['tip false', 'sub false', 'menu false']);
  assert.deepEqual(takeOverlayReplies(), [{ kind: 'res', id: 7, ok: true, result: { tapIndex: -1 } }], 'the sheet answers as a cancel');
  assert.equal(hasOverlay(), true, 'the toast stays');
  nodes = render(view);
  assert.deepEqual(['menu', 'sub', 'tip'].map(id => byId(nodes, id)), [undefined, undefined, undefined]);
  assert.ok(byId(nodes, 'note') && byId(nodes, 'confirm'), 'a manual popover and a dialog stay');
  assert.equal(dialogClosed, 0);

  assert.equal(globalThis.__lingxiaPageLeft(), false, 'nothing left to close');
});
