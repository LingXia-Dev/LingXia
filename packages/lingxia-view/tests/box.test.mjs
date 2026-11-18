import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { h, prepare, commit, dispatch, useBox, reset } from '../src/internal.js';
import { hasLocalUpdates } from '../src/runtime/hooks.js';
import { render as renderPage, textOf } from '@lingxia/view/testing';
import { decode } from './support/wire.mjs';

const render = view => { const out = decode(prepare(view, { env: {} })); commit(); return out; };
const byId = (nodes, id) => nodes.find(node => node.testId === id);
const size = (width, height) => JSON.stringify({ width, height });

test('useBox is null until the element is laid out, then holds its size and re-renders on a change', () => {
  reset();
  const view = () => {
    const box = useBox('chart');
    return h('page', {}, h('view', { box, 'data-testid': 'chart' }),
      h('text', { 'data-testid': 'out' }, box.value ? `${box.value.width}x${box.value.height}` : 'none'));
  };
  let nodes = render(view);
  assert.equal(byId(nodes, 'out').text, 'none');
  assert.equal(dispatch(byId(nodes, 'chart').props.onLayout, size(320, 180.125)), true);
  assert.equal(hasLocalUpdates(), true);
  nodes = render(view);
  assert.equal(byId(nodes, 'out').text, '320x180.125');
  dispatch(byId(nodes, 'chart').props.onLayout, size(320, 180.125));
  assert.equal(hasLocalUpdates(), false, 'the same size renders nothing again');
  dispatch(byId(nodes, 'chart').props.onLayout, size(200, 180.125));
  assert.equal(byId(render(view), 'out').text, '200x180.125');
});

test('onLayout is the primitive: each of its elements takes it, with or without a box', () => {
  reset();
  const seen = [];
  const on = tag => box => seen.push([tag, box.width, box.height]);
  const view = () => {
    const box = useBox('both');
    return h('page', {},
      h('view', { onLayout: on('view'), box, 'data-testid': 'view' }),
      h('text', { onLayout: on('text'), 'data-testid': 'text' }, 'T'),
      h('scroll', { onLayout: on('scroll'), 'data-testid': 'scroll' }),
      h('vector', { onLayout: on('vector'), 'data-testid': 'vector', paths: [{ d: 'M0 0L1 1' }] }),
      h('plot', { onLayout: on('plot'), 'data-testid': 'plot', kind: 'bar', values: [1, 2], 'aria-label': 'P' }),
      h('text', { 'data-testid': 'out' }, String(box.value?.width)));
  };
  let nodes = render(view);
  for (const tag of ['view', 'text', 'scroll', 'vector', 'plot']) dispatch(byId(nodes, tag).props.onLayout, size(10, 20));
  assert.deepEqual(seen, ['view', 'text', 'scroll', 'vector', 'plot'].map(tag => [tag, 10, 20]));
  assert.equal(byId(render(view), 'out').text, '10');
  assert.equal(byId(nodes, 'out').props.onLayout, undefined, 'an element without the prop sends nothing');
});

test('onLayout and box refuse other elements and values', () => {
  reset();
  assert.throws(() => prepare(() => h('page', {}, h('button', { onLayout() {} }, 'B')), { env: {} }), /UI_PROP/);
  assert.throws(() => prepare(() => h('page', {}, h('view', { onLayout: 1 })), { env: {} }), /UI_PROP: onLayout takes a function/);
  assert.throws(() => prepare(() => h('page', {}, h('view', { box: { value: null } })), { env: {} }), /UI_PROP: box takes the handle useBox/);
  assert.throws(() => prepare(() => { useBox(''); return h('page', {}); }, { env: {} }), /UI_PROP: useBox takes a name/);
});

const page = `import { useBox } from '@lingxia/view';
export function render() {
  const box = useBox('plot');
  return <view><view box={box} data-testid="plot" className="h-40" /><text data-testid="out">{box.value ? box.value.width : 'none'}</text></view>;
}
`;

test('screen.layout delivers a box to a compiled page and re-renders it', async () => {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-view-box-'));
  try {
    await writeFile(join(root, 'lxapp.json'), '{}');
    await writeFile(join(root, 'page.tsx'), page);
    const screen = await renderPage(join(root, 'page.tsx'));
    assert.equal(textOf(screen.getByTestId('out')), 'none');
    screen.layout(screen.getByTestId('plot'), { width: 343, height: 160 });
    assert.equal(textOf(screen.getByTestId('out')), '343');
  } finally { await rm(root, { recursive: true, force: true }); }
});
