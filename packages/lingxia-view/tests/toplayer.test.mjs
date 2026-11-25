import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, listTag, prepare, commit, dispatch, useLocal, reset } from '../src/internal.js';
import { escapeOverlay } from '../src/runtime/overlay.js';
import { interest, ownedPopovers, closeOrphans } from '../src/runtime/toplayer.js';
import { page } from '../src/runtime/state.js';
import { hasLocalUpdates } from '../src/runtime/hooks.js';
import { takeActionErrors } from '../src/runtime/actions.js';
import { compileUtilities } from '../compiler/tailwind.mjs';
import { configureClasses } from '../src/style/classes.js';
import { decode } from './support/wire.mjs';
import { Fragment } from '../src/runtime/nodes.js';

const render = (view, env = {}) => { const out = decode(prepare(view, { env })); commit(); return out; };
const byId = (nodes, id) => nodes.find(node => node.testId === id);

test('an open dialog is a top-layer box with a backdrop, a modal role and auto margins', () => {
  reset();
  let close = 0;
  const view = () => h('page', {}, h('view', { style: { flexDirection: 'row' } },
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

test('a <dialog> is modal; a popover with role="dialog" is not unless aria-modal says so', () => {
  reset();
  const view = modal => () => h('page', {},
    h('dialog', { open: true, 'aria-label': 'Delete?', 'data-testid': 'd' }, h('text', {}, 'Sure?')),
    h('view', { id: 'p', popover: true, open: true, role: 'dialog', 'aria-label': 'Filters', 'aria-modal': modal, 'data-testid': 'p' }, h('text', {}, 'Filters')));
  let nodes = render(view(undefined));
  assert.equal(byId(nodes, 'd').props.modal, 1);
  assert.equal(byId(nodes, 'p').props.role, 'dialog');
  assert.equal(byId(nodes, 'p').props.modal, undefined, 'a popover dialog is non-modal');
  reset();
  nodes = render(view(true));
  assert.equal(byId(nodes, 'p').props.modal, 1, 'aria-modal makes it modal');
});

test('role="dialog" needs a popover view, aria-modal a dialog, and a <dialog> stays modal', () => {
  const refused = (props, pattern) => { reset(); assert.throws(() => prepare(() => h('page', {}, h(props.tag, props.attrs))), pattern); };
  refused({ tag: 'view', attrs: { role: 'dialog', 'aria-label': 'Panel' } }, /role="dialog" goes on a popover <view>/);
  refused({ tag: 'view', attrs: { 'aria-modal': true } }, /aria-modal belongs on a popover with role="dialog"/);
  refused({ tag: 'dialog', attrs: { open: true, 'aria-label': 'Delete?', 'aria-modal': false } }, /a <dialog> is always modal/);
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

test('a link inside text is a popover invoker too', () => {
  reset();
  const view = () => h('page', {},
    h('text', {}, 'Due ', h('link', { popoverTarget: 'when', 'data-testid': 'more' }, 'Sunday')),
    h('view', { id: 'when', popover: true, 'data-testid': 'when' }, h('text', {}, 'Sunday 18:00')));
  let nodes = render(view);
  assert.equal(byId(nodes, 'more').props.stateExpanded, 0);
  dispatch(byId(nodes, 'more').props.onClick);
  nodes = render(view);
  assert.equal(byId(nodes, 'when').props.anchorTo, '--lx-popover-when');
  assert.equal(byId(nodes, 'more').props.anchorName, '--lx-popover-when');
  assert.equal(byId(nodes, 'more').props.stateExpanded, 1);
});

test('a link inside text is a hint trigger too', () => {
  reset();
  const view = () => h('page', {},
    h('text', {}, 'Due ', h('link', { interestFor: 'when', 'data-testid': 'more' }, 'Sunday')),
    h('view', { id: 'when', popover: 'hint', role: 'tooltip', 'data-testid': 'when' }, h('text', {}, 'Sunday 18:00')));
  assert.equal(byId(render(view), 'more').props.interestFor, 'when');
  assert.equal(interest('when', 0, true), true);
  const nodes = render(view);
  assert.equal(byId(nodes, 'more').props.anchorName, '--lx-popover-when');
  assert.equal(byId(nodes, 'more').props.ariaDescribedBy, 'when');
});

test('a trigger mid-sentence keeps its popover out of the paragraph', () => {
  reset();
  function When() {
    return h(Fragment, null, h('link', { popoverTarget: 'when', 'data-testid': 'more' }, 'Sunday'),
      h('view', { id: 'when', popover: true, 'data-testid': 'when' }, h('text', {}, 'Sunday 18:00')));
  }
  const view = () => h('page', {}, h('text', { 'data-testid': 'p' }, 'Due ', h(When), ' at noon'), h('text', {}, 'after'));
  let nodes = render(view);
  const runs = () => nodes.filter(node => node.parent?.testId === 'p').map(node => node.text);
  assert.deepEqual(runs(), ['Due ', 'Sunday', ' at noon']);
  assert.equal(byId(nodes, 'when'), undefined);
  dispatch(byId(nodes, 'more').props.onClick);
  nodes = render(view);
  assert.deepEqual(runs(), ['Due ', 'Sunday', ' at noon'], 'the open panel takes no inline space');
  const layer = byId(nodes, 'when').parent;
  assert.equal(layer.parent.tag, 'page');
  assert.match(layer.key, /^lx-lifted:/);
  assert.equal(byId(nodes, 'when').props.anchorTo, '--lx-popover-when');
  assert.equal(byId(nodes, 'more').props.anchorName, '--lx-popover-when');
  // The sibling after the paragraph keeps its name while the panel sits before it.
  assert.equal(nodes.find(node => node.text === 'after').props.childSlot, '1');
  // Inside text a fragment holds runs and top-layer parts only; a root paragraph has no place for them.
  function Boxed() { return h(Fragment, null, h('span', {}, 'a'), h('view', {})); }
  assert.throws(() => render(() => h('page', {}, h('text', {}, h(Boxed)))), /UI_TEXT: a component inside <text> returns a <view> beside its text/);
  assert.throws(() => render(() => h('text', {}, h(When))), /UI_ROOT: a <text> page root/);
});

test('a popover its invoker opened is owned; closing it as an orphan tells the page once', () => {
  reset();
  const toggled = [], shown = { value: true };
  const view = () => h('page', {},
    shown.value && h('button', { popoverTarget: 'menu', 'data-testid': 'more' }, 'More'),
    h('view', { id: 'menu', popover: 'manual', onToggle: open => toggled.push(open), 'data-testid': 'menu' }, h('text', {}, 'Menu')),
    h('view', { id: 'note', popover: 'manual', open: true, 'data-testid': 'note' }, h('text', {}, 'Note')));
  let nodes = render(view);
  assert.deepEqual(ownedPopovers(), [], 'one opened from code has no owner');
  dispatch(byId(nodes, 'more').props.onClick);
  render(view);
  assert.deepEqual(ownedPopovers(), ['menu']);
  // The invoker unmounts: the prepared frame still draws the popover, with no anchor name left.
  shown.value = false;
  nodes = decode(prepare(view, { env: {} }));
  assert.ok(byId(nodes, 'menu'));
  assert.equal(nodes.some(node => node.props.anchorName === '--lx-popover-menu'), false);
  assert.deepEqual(ownedPopovers(), ['menu'], 'read from the prepared frame');
  closeOrphans(['menu']);
  closeOrphans(['menu']);
  nodes = render(view);
  assert.equal(byId(nodes, 'menu'), undefined);
  assert.ok(byId(nodes, 'note'));
  assert.deepEqual(toggled, [true, false]);
});

test('an element two open popovers name carries both anchor names', () => {
  reset();
  const view = () => h('page', {},
    h('view', { id: 'field', 'data-testid': 'field' }, h('text', {}, 'Field')),
    h('view', { id: 'list', popover: 'manual', anchor: 'field', open: true, 'data-testid': 'list' }, h('text', {}, 'List')),
    h('view', { id: 'hint', popover: 'manual', anchor: 'field', open: true, 'data-testid': 'hint' }, h('text', {}, 'Hint')));
  const nodes = render(view);
  assert.equal(byId(nodes, 'field').props.anchorName, '--lx-popover-list\n--lx-popover-hint');
  assert.equal(byId(nodes, 'list').props.anchorTo, '--lx-popover-list');
  assert.equal(byId(nodes, 'hint').props.anchorTo, '--lx-popover-hint');
});

test('a popover anchored to an editor part names the editor and asks the core for its caret or selection', () => {
  reset();
  const view = () => h('page', {},
    h('textarea', { id: 'body', value: 'hi', 'aria-label': 'Body', 'data-testid': 'body' }),
    h('view', { id: 'mentions', popover: 'manual', anchor: 'body:caret', open: true, 'data-testid': 'mentions' }, h('text', {}, '@')),
    h('view', { id: 'bubble', popover: 'manual', anchor: 'body:selection', open: true, 'data-testid': 'bubble' }, h('text', {}, 'B')));
  const nodes = render(view);
  assert.equal(byId(nodes, 'body').props.anchorName, '--lx-popover-mentions\n--lx-popover-bubble');
  assert.equal(byId(nodes, 'mentions').props.anchorTo, '--lx-popover-mentions:caret');
  assert.equal(byId(nodes, 'bubble').props.anchorTo, '--lx-popover-bubble:selection');
});

test('a popover that names its anchor is placed against that element however it opens, and is owned by it', () => {
  reset();
  const open = { value: false };
  const view = () => h('page', {},
    h('view', { id: 'field', 'data-testid': 'field' }, h('text', {}, 'Field')),
    h('button', { popoverTarget: 'list', 'data-testid': 'more' }, 'More'),
    h('view', { id: 'list', popover: 'manual', anchor: 'field', open: open.value, 'data-testid': 'list' }, h('text', {}, 'List')));
  let nodes = render(view);
  assert.equal(byId(nodes, 'field').props.anchorName, undefined, 'closed: nothing is anchored');
  open.value = true;
  nodes = render(view);
  assert.equal(byId(nodes, 'list').props.anchorTo, '--lx-popover-list');
  assert.equal(byId(nodes, 'field').props.anchorName, '--lx-popover-list');
  assert.equal(byId(nodes, 'more').props.anchorName, undefined, 'the named element anchors it, not a button that targets it');
  assert.deepEqual(ownedPopovers(), ['list']);
  assert.throws(() => prepare(() => h('page', {}, h('view', { id: 'p', popover: 'manual', anchor: 'two words' })), { env: {} }), /UI_ID: anchor names an element id/);
  assert.throws(() => prepare(() => h('page', {}, h('view', { anchor: 'field' })), { env: {} }), /UI_PROP: anchor belongs to a popover/);
});

test('an invoker in a list row anchors the popover it opens and announces it open', () => {
  reset();
  const view = () => h('page', {},
    h('view', { id: 'menu', popover: true, 'data-testid': 'menu' }, h('text', {}, 'menu')),
    h(listTag, { count: 3, height: () => 40, row: i => h('button', { popoverTarget: 'menu', 'data-testid': `more-${i}`, 'aria-label': 'More' }) }));
  let nodes = render(view);
  assert.equal(byId(nodes, 'more-0').props.stateExpanded, 0);
  dispatch(byId(nodes, 'more-0').props.onClick);
  nodes = render(view);
  assert.equal(byId(nodes, 'menu').props.anchorTo, '--lx-popover-menu');
  assert.equal(byId(nodes, 'more-0').props.anchorName, '--lx-popover-menu', 'the invoker in the row carries the anchor it opened');
  assert.equal(byId(nodes, 'more-0').props.stateExpanded, 1);
});

test('a popover placed by its classes lays out clear of the bars; its backdrop still covers them', () => {
  reset();
  const view = () => h('page', {},
    h('view', { id: 'note', popover: true, open: true, 'data-testid': 'note' }, h('text', {}, 'note')));
  const nodes = render(view, { insets: { top: 56, right: 0, bottom: 49, left: 0 }, keyboard: { height: 0 } });
  const note = byId(nodes, 'note');
  assert.equal(note.props.anchorTo, undefined);
  const layer = note.parent;
  assert.equal(layer.props.paddingTop, 56);
  assert.equal(layer.props.paddingBottom, 49);
  const backdrop = byId(nodes, 'note-backdrop');
  assert.equal(backdrop.props.position, 'absolute', 'absolutely placed, the padding does not inset it');
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
  assert.equal(nodes.find(node => node.children.includes(byId(nodes, 'b'))).props.topLayer, 2, 'nor a close request');
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
  assert.throws(() => render(() => h('page', {}, h('view', { id: 'h', popover: 'manual', open: true, className: 'backdrop:bg-text/40' }))), /UI_STYLE: backdrop:/, 'a manual popover has no backdrop');
  configureClasses([]);
});

test('a dialog backdrop fades in and leaves with its dialog, on the dialog\'s timing unless backdrop: retimes it', async () => {
  reset();
  configureClasses(await compileUtilities(new Set(['transition', 'duration-300', 'ease-in', 'exit:opacity-0',
    'backdrop:exit:opacity-50', 'exit:backdrop:opacity-50', 'backdrop:starting:opacity-20', 'backdrop:duration-500', 'backdrop:exit:p-2'])));
  const backdropOf = className => byId(render(() => h('page', {}, h('dialog', { open: true, 'aria-label': 'x', 'data-testid': 'd', className }))), 'd-backdrop').props;
  const timing = props => { const { durationMs, easing } = JSON.parse(props.transition); return [durationMs, easing]; };
  const plain = backdropOf(undefined);
  assert.deepEqual([plain.startingStyle, plain.exitStyle], [undefined, undefined], 'a dialog that comes and goes at once takes its backdrop with it');
  assert.deepEqual(timing(plain), [200, [0, 0, 0.58, 1]]);
  const timed = backdropOf('transition duration-300 ease-in exit:opacity-0');
  assert.deepEqual(timing(timed), [300, [0.4, 0, 1, 1]], 'the dialog\'s duration and easing');
  assert.deepEqual([timed.startingStyle, timed.exitStyle], [undefined, '[58,0]'], 'it fades out with a dialog that fades out');
  assert.equal(backdropOf('exit:opacity-0').exitStyle, '[58,0]', 'on the default timing when the dialog sets none');
  const own = backdropOf('transition duration-300 ease-in backdrop:starting:opacity-20 backdrop:exit:opacity-50 backdrop:duration-500');
  assert.deepEqual(timing(own), [500, [0.4, 0, 1, 1]], 'backdrop:duration-* wins; the easing stays the dialog\'s');
  assert.deepEqual([own.startingStyle, own.exitStyle], ['[58,0.2]', '[58,0.5]']);
  assert.equal(backdropOf('exit:backdrop:opacity-50').exitStyle, '[58,0.5]', 'either order of the variants');
  assert.throws(() => backdropOf('backdrop:exit:p-2'), /UI_STYLE: backdrop:exit: supports paint properties only/);
  configureClasses([]);
});

test('a menu item that opens a submenu keeps its menu; a choice in the submenu closes both; Escape closes the innermost', () => {
  reset();
  const log = [];
  // The submenu sits beside its menu in the tree, as a kit portals it.
  const view = () => h('page', {},
    h('button', { popoverTarget: 'menu', 'data-testid': 'more' }, 'More'),
    h('view', { id: 'menu', popover: true, role: 'menu', 'aria-label': 'Actions', onToggle: open => log.push(`menu=${open}`), 'data-testid': 'menu' },
      h('button', { role: 'menuitem', 'data-testid': 'rename' }, 'Rename'),
      h('button', { role: 'menuitem', popoverTarget: 'move', 'data-testid': 'move-to' }, 'Move to'),
      h('button', { role: 'menuitem', popoverTarget: 'share', 'data-testid': 'share-with' }, 'Share with')),
    h('view', { id: 'move', popover: true, role: 'menu', 'aria-label': 'Move to', onToggle: open => log.push(`move=${open}`), 'data-testid': 'move' },
      h('button', { role: 'menuitem', 'data-testid': 'inbox', onClick: () => log.push('inbox') }, 'Inbox'),
      h('button', { role: 'menuitem', popoverTarget: 'deeper', 'data-testid': 'archive' }, 'Archive')),
    h('view', { id: 'deeper', popover: true, role: 'menu', 'aria-label': 'Archive', 'data-testid': 'deeper' },
      h('button', { role: 'menuitem', 'data-testid': 'year' }, '2026')),
    h('view', { id: 'share', popover: true, role: 'menu', 'aria-label': 'Share with', 'data-testid': 'share' },
      h('button', { role: 'menuitem', 'data-testid': 'team' }, 'Team')));
  const press = id => { dispatch(byId(render(view), id).props.onClick); return render(view); };
  let nodes = press('more');
  nodes = press('move-to');
  assert.ok(byId(nodes, 'menu'), 'the menu stays under its submenu');
  assert.ok(byId(nodes, 'move'));
  assert.equal(byId(nodes, 'move-to').props.stateExpanded, 1);
  assert.equal(byId(nodes, 'move').props.anchorTo, '--lx-popover-move');
  assert.equal(byId(nodes, 'move-to').props.anchorName, '--lx-popover-move');
  assert.ok(byId(nodes, 'menu-backdrop'));
  assert.equal(byId(nodes, 'move-backdrop'), undefined, 'a submenu leaves its menu\'s items pressable');
  // Another item of the menu swaps the submenu.
  nodes = press('share-with');
  assert.ok(byId(nodes, 'menu') && byId(nodes, 'share'));
  assert.equal(byId(nodes, 'move'), undefined);
  nodes = press('move-to');
  nodes = press('archive');
  assert.ok(byId(nodes, 'menu') && byId(nodes, 'move') && byId(nodes, 'deeper'), 'three levels stay open');
  assert.equal(escapeOverlay(), true);
  nodes = render(view);
  assert.ok(byId(nodes, 'move') && !byId(nodes, 'deeper'), 'Escape closes the innermost');
  assert.equal(escapeOverlay(), true);
  nodes = render(view);
  assert.ok(byId(nodes, 'menu') && !byId(nodes, 'move'));
  nodes = press('move-to');
  log.length = 0;
  nodes = press('inbox');
  assert.deepEqual(log, ['inbox', 'move=false', 'menu=false'], 'a choice closes the chain, innermost first');
  assert.ok(!byId(nodes, 'menu') && !byId(nodes, 'move'));
  // A press outside reaches the menu's backdrop and takes the submenu with it.
  press('more');
  nodes = press('move-to');
  log.length = 0;
  nodes = press('menu-backdrop');
  assert.deepEqual(log, ['move=false', 'menu=false']);
  assert.ok(!byId(nodes, 'menu') && !byId(nodes, 'move'));
  // Pressing the item again closes only its submenu.
  press('more');
  press('move-to');
  nodes = press('move-to');
  assert.ok(byId(nodes, 'menu') && !byId(nodes, 'move'));
  assert.equal(byId(nodes, 'move-to').props.stateExpanded, 0);
});

test('a menu item with keepOpen runs its click and leaves its menu open', () => {
  reset();
  const calls = [];
  const view = () => h('page', {},
    h('button', { popoverTarget: 'menu', 'data-testid': 'more' }, 'More'),
    h('view', { id: 'menu', popover: true, role: 'menu', 'aria-label': 'View', onToggle: open => calls.push(open), 'data-testid': 'menu' },
      h('button', { role: 'menuitem', keepOpen: true, 'data-testid': 'zoom', onClick: () => calls.push('zoom') }, 'Zoom in'),
      h('button', { role: 'menuitem', keepOpen: false, 'data-testid': 'reset', onClick: () => calls.push('reset') }, 'Reset')));
  dispatch(byId(render(view), 'more').props.onClick);
  let nodes = render(view);
  dispatch(byId(nodes, 'zoom').props.onClick);
  nodes = render(view);
  assert.ok(byId(nodes, 'menu'), 'the menu stays');
  dispatch(byId(nodes, 'zoom').props.onClick);
  dispatch(byId(render(view), 'reset').props.onClick);
  assert.equal(byId(render(view), 'menu'), undefined, 'an item without it still closes the menu');
  assert.deepEqual(calls, [true, 'zoom', 'zoom', 'reset', false]);
  assert.throws(() => render(() => h('page', {}, h('button', { keepOpen: true }, 'X'))), /UI_PROP: keepOpen/);
  assert.throws(() => render(() => h('page', {}, h('view', { id: 'm', popover: true, open: true }, h('button', { role: 'menuitem', keepOpen: 1 }, 'X')))), /UI_PROP: keepOpen/);
});

test('onCloseRequest hears Escape before a popover closes and can refuse it; a press outside still closes', () => {
  reset();
  const calls = [];
  let allow = false;
  const view = () => h('page', {},
    h('button', { popoverTarget: 'form', 'data-testid': 'edit' }, 'Edit'),
    h('view', { id: 'form', popover: true, onToggle: open => calls.push(open), onCloseRequest: () => { calls.push('request'); return allow; }, 'data-testid': 'form' }, h('text', {}, 'Draft')));
  dispatch(byId(render(view), 'edit').props.onClick);
  render(view);
  assert.equal(escapeOverlay(), true, 'a refused request is still taken');
  assert.ok(byId(render(view), 'form'), 'returning false keeps it open');
  dispatch(byId(render(view), 'form-backdrop').props.onClick);
  assert.equal(byId(render(view), 'form'), undefined, 'a press outside does not ask');
  assert.deepEqual(calls, [true, 'request', false]);
  dispatch(byId(render(view), 'edit').props.onClick);
  render(view);
  allow = undefined;
  assert.equal(escapeOverlay(), true);
  assert.equal(byId(render(view), 'form'), undefined, 'anything but false lets it close');
  assert.deepEqual(calls.slice(3), [true, 'request', false]);
  assert.throws(() => render(() => h('page', {}, h('view', { id: 'p', popover: 'manual', onCloseRequest: () => false }))), /UI_PROP: onCloseRequest/);
  assert.throws(() => render(() => h('page', {}, h('view', { id: 'p', popover: true, onCloseRequest: true }))), /UI_PROP: onCloseRequest/);
  assert.throws(() => render(() => h('page', {}, h('view', { onCloseRequest: () => false }))), /add popover/);
});

test('interestFor shows a hint on the core\'s word; a hint closes other hints, an auto popover closes hints', () => {
  reset();
  const view = () => h('page', {},
    h('button', { interestFor: 'tip', 'data-testid': 'info', 'aria-label': 'About' }),
    h('button', { interestFor: 'other', 'data-testid': 'more-info', 'aria-label': 'More' }),
    h('button', { popoverTarget: 'menu', 'data-testid': 'menu-button' }, 'Menu'),
    h('view', { id: 'tip', popover: 'hint', role: 'tooltip', 'data-testid': 'tip' }, h('text', {}, 'Storage you have left')),
    h('view', { id: 'other', popover: 'hint', role: 'tooltip', 'data-testid': 'other' }, h('text', {}, 'More')),
    h('view', { id: 'menu', popover: true, 'data-testid': 'menu' }));
  let nodes = render(view);
  const info = byId(nodes, 'info');
  assert.equal(info.props.interestFor, 'tip');
  assert.equal(info.props.stateExpanded, undefined, 'a hint trigger is not a disclosure');
  assert.equal(interest('tip', 0, true), true);
  nodes = render(view);
  assert.ok(byId(nodes, 'tip'));
  assert.equal(byId(nodes, 'tip-backdrop'), undefined, 'a hint takes no taps');
  assert.equal(byId(nodes, 'tip').props.anchorTo, '--lx-popover-tip');
  assert.equal(byId(nodes, 'info').props.anchorName, '--lx-popover-tip');
  assert.equal(byId(nodes, 'info').props.ariaDescribedBy, 'tip', 'the shown hint describes its trigger');
  assert.equal(interest('other', 0, true), true);
  nodes = render(view);
  assert.equal(byId(nodes, 'tip'), undefined, 'a new hint closes the previous one');
  dispatch(byId(nodes, 'menu-button').props.onClick);
  nodes = render(view);
  assert.ok(byId(nodes, 'menu'));
  assert.equal(byId(nodes, 'other'), undefined, 'an auto popover closes hints');
  assert.equal(interest('tip', 0, true), true);
  nodes = render(view);
  assert.ok(byId(nodes, 'menu'), 'a hint leaves auto popovers open');
  assert.equal(escapeOverlay(), true);
  assert.equal(byId(render(view), 'tip'), undefined, 'Escape closes the newest, the hint');
  assert.equal(interest('tip', 0, false), false, 'already hidden');
  assert.throws(() => render(() => h('page', {}, h('button', { interestFor: 'nope' }, 'X'))), /UI_ID: interestFor "nope"/);
});

test('closeAfter rides a manual popover\'s layer and its expiry closes it as a light dismiss does', async () => {
  reset();
  const { closeAfterExpired } = await import('../src/runtime/toplayer.js');
  const calls = [];
  const view = () => {
    const shown = useLocal('shown', true);
    return h('page', {}, h('view', { id: 'toast', popover: 'manual', open: shown.value, closeAfter: 4000,
      onToggle: next => { calls.push(next); shown.set(next); }, 'data-testid': 'toast' }, h('text', {}, 'Saved')));
  };
  let nodes = render(view);
  const layer = nodes.find(node => node.children.includes(byId(nodes, 'toast')));
  assert.equal(layer.props.closeAfter, 4000);
  assert.equal(closeAfterExpired('other'), false);
  assert.equal(closeAfterExpired('toast'), true);
  assert.deepEqual(calls, [false]);
  nodes = render(view);
  assert.equal(byId(nodes, 'toast'), undefined);
  assert.equal(closeAfterExpired('toast'), false, 'a closed popover expires no more');
  for (const bad of [
    () => h('view', { id: 't', popover: 'auto', closeAfter: 1000 }),
    () => h('view', { id: 't', popover: 'manual', closeAfter: 0 }),
    () => h('view', { id: 't', popover: 'manual', closeAfter: 60001 }),
    () => h('view', { id: 't', popover: 'manual', closeAfter: 1.5 }),
    () => h('view', { closeAfter: 1000 }),
  ]) assert.throws(() => render(() => h('page', {}, bad())), /UI_PROP/);
});


test('a panel docked to an edge runs to it and pads its content by the inset; elsewhere the layer clears the bars', async () => {
  reset();
  configureClasses(await compileUtilities(new Set(['mt-auto', 'mb-0', 'mt-0', 'w-full', 'h-full', 'max-w-[640px]', 'px-5', 'pb-5', 'ms-0', 'me-auto', 'absolute', 'inset-0', 'm-0', 'w-[60px]', 'backdrop:bg-scrim'])));
  const space = [23, 5, 0];
  const at = (className, env, tag = 'dialog') => {
    const panel = () => tag === 'dialog'
      ? h('dialog', { open: true, 'aria-label': 'Sheet', 'data-testid': 'p', className }, h('text', {}, 'x'))
      : h('view', { id: 'm', popover: 'manual', open: true, 'data-testid': 'p', className }, h('text', {}, 'x'));
    reset();
    const nodes = render(() => h('page', {}, panel()), env);
    const p = byId(nodes, 'p');
    const layer = nodes.find(node => node.children.includes(p));
    const pad = props => ['Top', 'Right', 'Bottom', 'Left'].map(side => JSON.stringify(props[`padding${side}`] ?? null)).join(' ');
    return { layer: pad(layer.props), panel: pad(p.props), raw: p.props };
  };
  const phone = { insets: { top: 47, right: 0, bottom: 34, left: 0 }, keyboard: { height: 0 }, viewport: { width: 390, height: 844 } };
  const sheet = 'mt-auto mb-0 w-full max-w-[640px] px-5 pb-5';

  // Bottom sheet: the bottom bleeds, its content clears the home indicator; the top stays clear.
  let got = at(sheet, phone);
  assert.equal(got.layer, '47 null null null', 'a phone-wide sheet touches the sides too, which have no inset');
  assert.deepEqual(got.raw.paddingBottom, [space[0], space[1], 34]);
  // A centred dialog keeps clear of every bar, its own padding untouched.
  assert.equal(at(undefined, phone).layer, '47 0 34 0');
  assert.equal(at(undefined, phone).panel, 'null null null null');
  // Over an open keyboard the sheet sits on the keyboard: nothing to clear there.
  got = at(sheet, { ...phone, keyboard: { height: 300 } });
  assert.deepEqual(got.raw.paddingBottom, space);
  // Landscape: a 640 px sheet does not reach the side bars, which the layer clears; a full-width one pads past them.
  const landscape = { insets: { top: 0, right: 44, bottom: 21, left: 44 }, keyboard: { height: 0 }, viewport: { width: 844, height: 390 } };
  got = at(sheet, landscape);
  assert.equal(got.layer, '0 44 null 44');
  assert.deepEqual([got.raw.paddingLeft, got.raw.paddingRight, got.raw.paddingBottom], [space, space, [space[0], space[1], 21]]);
  got = at(sheet, { ...landscape, viewport: { width: 600, height: 390 } });
  assert.equal(got.layer, '0 null null null');
  assert.deepEqual([got.raw.paddingLeft, got.raw.paddingRight], [[space[0], space[1], 44], [space[0], space[1], 44]]);
  // A start drawer, full height: start, top and bottom bleed; the far side clears its bar. In RTL start is the right.
  got = at('ms-0 me-auto h-full px-5', landscape);
  assert.equal(got.layer, 'null 44 null null');
  assert.deepEqual(got.raw.paddingLeft, [space[0], space[1], 44]);
  assert.equal(got.raw.paddingTop, undefined, 'no top inset in landscape, nothing added');
  got = at('ms-0 me-auto h-full px-5', { ...landscape, direction: 'rtl' });
  assert.equal(got.layer, 'null null null 44');
  assert.deepEqual(got.raw.paddingRight, [space[0], space[1], 44]);
  got = at('ms-0 me-auto h-full px-5', phone);
  assert.deepEqual([got.raw.paddingTop, got.raw.paddingBottom], [47, 34]);
  // Top-docked: the status bar's inset pads it, the bottom stays clear.
  got = at('mt-0 w-full', phone);
  assert.equal(got.layer, 'null null 34 null');
  assert.equal(got.raw.paddingTop, 47);
  // A small box set in a corner is not docked: it stays clear of the bars.
  got = at('mt-0 ms-0 w-[60px]', phone, 'popover');
  assert.equal(got.layer, '47 0 34 0');
  assert.equal(got.panel, 'null null null null');
  // A full-bleed container is docked on every side and padded on each, so a sheet at its foot
  // stops above the home indicator: a sheet that runs to the edge is the panel itself.
  got = at('absolute inset-0 m-0', phone, 'popover');
  assert.equal(got.layer, 'null null null null');
  assert.deepEqual([got.raw.paddingTop, got.raw.paddingBottom], [47, 34]);
  // A docked menu as a light-dismiss popover sheet: it runs to the edge over its own scrim.
  reset();
  const nodes = render(() => h('page', {}, h('view', { id: 'menu', popover: true, open: true, role: 'menu', 'aria-label': 'Row',
    'data-testid': 'menu', className: `${sheet} backdrop:bg-scrim` }, h('text', {}, 'x'))), phone);
  const menu = byId(nodes, 'menu'), backdrop = byId(nodes, 'menu-backdrop');
  assert.deepEqual(menu.props.paddingBottom, [space[0], space[1], 34]);
  assert.equal(menu.parent.props.paddingBottom, undefined, 'the layer leaves the bottom to the sheet');
  assert.equal(backdrop.props.background, 'scrim');
  assert.deepEqual([backdrop.props.insetTop, backdrop.props.insetBottom], [0, 0], 'the scrim covers the whole layer');
  const plain = byId(render(() => h('page', {}, h('view', { id: 'n', popover: true, open: true, 'data-testid': 'n' }, h('text', {}, 'x'))), phone), 'n-backdrop');
  assert.equal(plain.props.background, '#00000000', 'without backdrop: a popover\'s backdrop stays clear');
  configureClasses([]);
});
