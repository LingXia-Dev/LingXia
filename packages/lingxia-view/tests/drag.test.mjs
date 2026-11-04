import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, dispatch, useLocal, reset } from '../src/internal.js';
import { decode } from './support/wire.mjs';

const render = (view, env = {}) => { const out = decode(prepare(view, { env })); commit(); return out; };
const byId = (nodes, id) => nodes.find(node => node.testId === id);
const event = (phase, index = 0, velocity = 0) => JSON.stringify({ phase, index, velocity });

test('drag sends its axis, points and bound index; its events reach the callbacks and the cell', () => {
  reset();
  const seen = [];
  let cell;
  const view = () => {
    cell = useLocal('at', 1);
    return h('page', {}, h('view', { drag: 'y', dragSnapPoints: [0, '50%', 240], bind: cell, 'aria-label': 'Panel', 'data-testid': 'p',
      onDragStart: () => seen.push('start'), onDragEnd: info => seen.push(info) }));
  };
  const panel = byId(render(view), 'p');
  assert.deepEqual(JSON.parse(panel.props.drag), { axis: 'y', points: [0, '50%', 240], index: 1 });
  const handle = panel.props.onDragEvent;
  assert.equal(dispatch(handle, event('start')), true);
  assert.equal(dispatch(handle, event('end', 2, 812.5)), true);
  assert.deepEqual(seen, ['start', { index: 2, velocity: 812.5 }]);
  assert.equal(cell.value, 1, 'the cell follows the settle, not the release');
  dispatch(handle, event('settle', 2));
  assert.equal(cell.value, 2);
  assert.equal(JSON.parse(byId(render(view), 'p').props.drag).index, 2);
  dispatch(byId(render(view), 'p').props.onDragEvent, event('settle', 9));
  assert.equal(cell.value, 2, 'an index past the points is ignored');
});

test('drag props are checked where they are written', () => {
  reset();
  const panel = props => () => h('page', {}, h('view', { 'aria-label': 'Panel', ...props }));
  assert.throws(() => render(panel({ drag: 'z' })), /UI_PROP: drag is "x" or "y"/);
  assert.throws(() => render(panel({ drag: 'x', dragSnapPoints: [] })), /dragSnapPoints/);
  assert.throws(() => render(panel({ drag: 'x', dragSnapPoints: Array(9).fill(0) })), /1\.\.8/);
  assert.throws(() => render(panel({ drag: 'x', dragSnapPoints: ['50px'] })), /dragSnapPoints/);
  assert.throws(() => render(panel({ dragSnapPoints: [0] })), /belongs to an element with drag/);
  assert.throws(() => render(() => h('page', {}, h('view', { drag: 'x' }))), /UI_A11Y/);
  assert.throws(() => render(() => { const at = useLocal('at', 3); return h('page', {}, h('view', { drag: 'x', dragSnapPoints: [0, 10], bind: at, 'aria-label': 'P' })); }),
    /out of range/);
  assert.throws(() => render(panel({ drag: 'x', bind: { value: 0 } })), /mounted number useLocal cell/);
});

test('a dialog that settles hidden closes; closedby="none" keeps it; its backdrop fades with it', () => {
  reset();
  let closes = 0;
  const sheet = closedby => () => h('page', {}, h('dialog', { open: true, onClose: () => closes++, closedby, drag: 'y',
    dragSnapPoints: [0, '100%'], 'aria-label': 'Filters', 'data-testid': 'sheet' }));
  const nodes = render(sheet('any'));
  assert.equal(byId(nodes, 'sheet-backdrop').props.dragScrim, 1);
  dispatch(byId(nodes, 'sheet').props.onDragEvent, JSON.stringify({ phase: 'settle', index: 1, velocity: 0, hidden: false }));
  assert.equal(closes, 0, 'only a hidden settle closes');
  dispatch(byId(render(sheet('any')), 'sheet').props.onDragEvent, JSON.stringify({ phase: 'settle', index: 1, velocity: 0, hidden: true }));
  assert.equal(closes, 1);
  dispatch(byId(render(sheet('none')), 'sheet').props.onDragEvent, JSON.stringify({ phase: 'settle', index: 1, velocity: 0, hidden: true }));
  assert.equal(closes, 1);
  assert.equal(byId(render(() => h('page', {}, h('dialog', { open: true, 'aria-label': 'Plain', 'data-testid': 'p' }))), 'p-backdrop').props.dragScrim, undefined);
});

test('a swipe row names the controls behind it; they carry their ids', () => {
  reset();
  const nodes = render(() => h('page', {}, h('stack', {},
    h('button', { id: 'del-1', 'data-testid': 'del', onClick: () => {} }, 'Delete'),
    h('button', { 'data-testid': 'row', drag: 'x', dragSnapPoints: [0, -88], 'aria-actions': 'del-1', onClick: () => {} }, 'Groceries'))));
  assert.equal(byId(nodes, 'del').props.elementId, 'del-1');
  assert.equal(byId(nodes, 'row').props.ariaActions, 'del-1');
  assert.throws(() => render(() => h('page', {}, h('button', { 'aria-actions': 'x', onClick: () => {} }, 'Row'))), /aria-actions belongs to an element with drag/);
  assert.throws(() => render(() => h('page', {}, h('button', { drag: 'x', dragSnapPoints: [0, 1], 'aria-actions': '  ', onClick: () => {} }, 'Row'))), /1\.\.8 element ids/);
  assert.throws(() => render(() => h('page', {}, h('button', { id: '', onClick: () => {} }, 'Row'))), /UI_ID/);
});

test('bind on a snapping scroll sends its snap index and takes the one it settles on', () => {
  reset();
  let cell;
  const view = () => { cell = useLocal('page', 1); return h('page', {}, h('scroll', { 'data-testid': 's', bind: cell })); };
  const scroll = byId(render(view), 's');
  assert.equal(scroll.props.snapIndex, 1);
  assert.equal(dispatch(scroll.props.onSnapChange, '2'), true);
  assert.equal(cell.value, 2);
  dispatch(byId(render(view), 's').props.onSnapChange, '-1');
  assert.equal(cell.value, 2, 'a negative index is ignored');
  assert.throws(() => render(() => h('page', {}, h('scroll', { bind: { value: 0 } }))), /snap index/);
});
