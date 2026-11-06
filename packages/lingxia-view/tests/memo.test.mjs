import { test, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { decode } from './support/wire.mjs';
import { h, List, describe, prepare, commit, discard, reset, dispatch, useLocal, useView, createView } from '../src/internal.js';
import { stableClosure } from '../src/runtime/memo.js';

beforeEach(() => reset());

const texts = buffer => decode(buffer).filter(node => node.tag === 'text').map(node => node.text);
const view = (data = {}, env = {}) => createView(data, env);

test('a component re-runs only when its props, place, state or environment change', () => {
  const runs = { Static: 0, Shown: 0 };
  function Static({ label }) { runs.Static++; return h('text', {}, label); }
  function Shown({ value }) { runs.Shown++; return h('text', {}, String(value)); }
  const render = ({ data }) => h('column', {}, h(Static, { label: 'fixed' }), h(Shown, { value: data.n }));
  describe(render, view({ n: 1 }));
  describe(render, view({ n: 2 }));
  assert.deepEqual(runs, { Static: 1, Shown: 2 });
  assert.deepEqual(texts(describe(render, view({ n: 3 }))), ['fixed', '3']);
  assert.deepEqual(runs, { Static: 1, Shown: 3 });
  describe(render, view({ n: 3 }, { theme: 'dark' }));
  assert.deepEqual(runs, { Static: 2, Shown: 4 }, 'a new environment re-runs everything');
});

test('a recalled component keeps its state, and re-runs when a cell it declared or read changes', () => {
  let own, outer, runs = 0;
  function Counter({ shared }) { runs++; own = useLocal('n', 0); return h('text', {}, `${own.value}/${shared.value}`); }
  const render = () => { outer = useLocal('outer', 'a'); return h('column', {}, h(Counter, { shared: outer })); };
  describe(render, view());
  describe(render, view());
  assert.equal(runs, 1);
  own.set(1);
  assert.deepEqual(texts(describe(render, view())), ['1/a']);
  outer.set('b');
  assert.deepEqual(texts(describe(render, view())), ['1/b']);
  assert.equal(runs, 3);
  describe(render, view());
  assert.equal(runs, 3, 'state survives the recall');
  own.set(2);
  assert.deepEqual(texts(describe(render, view())), ['2/b']);
});

test('a component reading the View, or marked volatile, re-runs on every render', () => {
  let reads = 0, volatile = 0;
  function Reader() { reads++; return h('text', {}, String(useView().data.n)); }
  function Clock() { volatile++; return h('text', {}, 'now'); }
  Clock.__lxVolatile = true;
  const render = () => h('column', {}, h(Reader), h(Clock));
  describe(render, view({ n: 1 }));
  assert.deepEqual(texts(describe(render, view({ n: 2 }))), ['2', 'now']);
  assert.deepEqual([reads, volatile], [2, 2]);
});

test('callbacks in a recalled subtree still dispatch, and a discarded render keeps the committed one', () => {
  let clicks = 0, runs = 0;
  const onClick = () => { clicks++; };
  function Button({ label }) { runs++; return h('button', { onClick }, label); }
  const render = ({ data }) => h('column', {}, h(Button, { label: 'Go' }), h('text', {}, String(data.n)));
  describe(render, view({ n: 1 }));
  prepare(render, view({ n: 2 })); discard();
  const output = decode(describe(render, view({ n: 3 })));
  assert.equal(runs, 1);
  assert.ok(dispatch(output.find(node => node.tag === 'button').props.onClick));
  assert.equal(clicks, 1);
});

test('equal JSX children and plain props compare by value', () => {
  let runs = 0;
  function Card({ style, children }) { runs++; return h('column', { style }, children); }
  const render = ({ data }) => h('column', {}, h(Card, { style: { padding: 4 } }, h('text', {}, 'body')), h('text', {}, String(data.n)));
  describe(render, view({ n: 1 }));
  describe(render, view({ n: 2 }));
  assert.equal(runs, 1);
  const changed = ({ data }) => h('column', {}, h(Card, { style: { padding: 4 } }, h('text', {}, `body ${data.n}`)));
  describe(changed, view({ n: 3 }));
  assert.equal(runs, 2);
});

test('a recalled list keeps rows that hold state rendering against their cells', () => {
  const items = Object.freeze(['a', 'b']), cells = new Map();
  const renderItem = item => { const cell = useLocal('on', false); cells.set(item, cell); return h('text', {}, `${item}:${cell.value}`); };
  function Rows() { return h(List, { items, getKey: String, renderItem }); }
  const render = ({ data }) => h('column', {}, h(Rows), h('text', {}, String(data.n)));
  const id = decode(describe(render, view({ n: 0 }))).find(node => node.tag === 'list').props.listId;
  const windows = { [id]: { start: 0, end: 2, leading: 0, trailing: 0 } };
  prepare(render, view({ n: 1 }), windows); commit();
  prepare(render, view({ n: 1 }), windows); commit();
  cells.get('b').set(true);
  const output = decode(prepare(render, view({ n: 2 }), windows)); discard();
  assert.deepEqual(output.filter(node => node.tag === 'text').map(node => node.text), ['a:false', 'b:true', '2']);
});

test('a closure keeps its identity while what it captured is unchanged', () => {
  const made = [];
  const render = ({ data }) => {
    const pick = stableClosure('site', () => data.k, [data.k]);
    made.push(pick);
    return h('text', {}, 'x');
  };
  describe(render, view({ k: 1 }));
  describe(render, view({ k: 1, other: 2 }));
  describe(render, view({ k: 2 }));
  assert.equal(made[0], made[1]);
  assert.notEqual(made[1], made[2]);
});

test('unchanged parts of new data keep their identity', () => {
  const first = view({ list: [{ id: 1 }], meta: { tick: 1 } }).data;
  const second = view({ list: [{ id: 1 }], meta: { tick: 2 } }).data;
  assert.equal(first.list, second.list);
  assert.notEqual(first.meta, second.meta);
  assert.ok(Object.isFrozen(second.meta));
});
