import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { h, prepare, commit, dispatch, reset } from '../src/internal.js';
import { render as renderPage, textOf } from '@lingxia/view/testing';
import { decode } from './support/wire.mjs';

const render = view => { const out = decode(prepare(view, { env: {} })); commit(); return out; };
const byId = (nodes, id) => nodes.find(node => node.testId === id);

test('onPointer hears each event as a frozen object; pointerAxis travels only when it is not both', () => {
  reset();
  const seen = [];
  const view = () => h('page', {},
    h('view', { onPointer: event => seen.push(event), 'data-testid': 'both' }),
    h('vector', { onPointer() {}, pointerAxis: 'x', 'data-testid': 'x', paths: [{ d: 'M0 0L1 1' }] }),
    h('button', { onPointer() {}, pointerAxis: 'y', 'data-testid': 'y' }, 'B'),
    h('view', { 'data-testid': 'plain' }));
  const nodes = render(view);
  assert.equal(byId(nodes, 'both').props.pointerAxis, undefined);
  assert.equal(byId(nodes, 'x').props.pointerAxis, 'x');
  assert.equal(byId(nodes, 'y').props.pointerAxis, 'y');
  assert.equal(byId(nodes, 'plain').props.onPointer, undefined, 'an element without the prop sends nothing');
  const event = { phase: 'move', x: 12.5, y: -3, width: 300, height: 120, pointer: 'touch' };
  assert.equal(dispatch(byId(nodes, 'both').props.onPointer, JSON.stringify(event)), true);
  assert.deepEqual(seen, [event]);
  assert.ok(Object.isFrozen(seen[0]));
});

test('onPointer and pointerAxis refuse other elements and values', () => {
  reset();
  const page = node => () => prepare(() => h('page', {}, node()), { env: {} });
  assert.throws(page(() => h('text', { onPointer() {} }, 'T')), /UI_PROP: onPointer is not a property of <text>/);
  assert.throws(page(() => h('view', { onPointer: true })), /UI_PROP: onPointer takes a function/);
  assert.throws(page(() => h('view', { pointerAxis: 'x' })), /UI_PROP: onPointer takes a function; pointerAxis goes with it/);
  assert.throws(page(() => h('view', { onPointer() {}, pointerAxis: 'z' })), /UI_PROP: pointerAxis must be "x", "y" or "both"/);
});

test('an onPointer element may report the value its handler steps; other elements still may not', () => {
  reset();
  const node = byId(render(() => h('page', {}, h('view', { onPointer() {}, 'aria-label': 'Sales', 'aria-valuenow': 2, 'aria-valuemin': 0,
    'aria-valuemax': 11, 'aria-valuetext': 'March: 42', 'data-testid': 'plot' }))), 'plot');
  assert.equal(node.props.ariaValueNow, 2);
  assert.equal(node.props.ariaValueMax, 11);
  assert.equal(node.props.ariaValueText, 'March: 42');
  assert.throws(() => prepare(() => h('page', {}, h('view', { 'aria-valuenow': 2 })), { env: {} }),
    /UI_PROP: aria-valuenow, -valuemin, -valuemax and -valuetext belong on role="progressbar", "spinbutton" or "combobox", or an onPointer element/);
});

test('an older host gets neither field', () => {
  reset();
  const previous = globalThis.__lingxiaViewHost;
  globalThis.__lingxiaViewHost = { maxField: 244 };
  try {
    const node = byId(render(() => h('page', {}, h('view', { onPointer() {}, pointerAxis: 'x', 'data-testid': 'v' }))), 'v');
    assert.equal(node.props.onPointer, undefined);
    assert.equal(node.props.pointerAxis, undefined);
  } finally { globalThis.__lingxiaViewHost = previous; }
});

const page = `import { useLocal } from '@lingxia/view';
export function render() {
  const at = useLocal('at', 'none');
  return <view>
    <view data-testid="plot" className="h-40" pointerAxis="x" onPointer={e => at.set(e.phase + ' ' + Math.round(e.x / e.width * 100) + '%')} />
    <text data-testid="out">{at.value}</text>
  </view>;
}
`;

test('screen.pointer delivers an event to a compiled page and re-renders it', async () => {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-view-pointer-'));
  try {
    await writeFile(join(root, 'lxapp.json'), '{}');
    await writeFile(join(root, 'page.tsx'), page);
    const screen = await renderPage(join(root, 'page.tsx'));
    screen.pointer(screen.getByTestId('plot'), { phase: 'move', x: 80, y: 10, width: 320, height: 160 });
    assert.equal(textOf(screen.getByTestId('out')), 'move 25%');
  } finally { await rm(root, { recursive: true, force: true }); }
});
