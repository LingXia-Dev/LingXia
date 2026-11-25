import { test, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { decode } from './support/wire.mjs';
import { h, List, describe, prepare, commit, discard, reset, dispatch, useLocal, useView, useEnv, createView, configureClasses } from '../src/internal.js';
import { stableClosure } from '../src/runtime/memo.js';

beforeEach(() => reset());

const texts = buffer => decode(buffer).filter(node => node.tag === 'text').map(node => node.text);
const view = (data = {}, env = {}) => createView(data, env);

test('a component re-runs only when its props, place, state or environment change', () => {
  const runs = { Static: 0, Shown: 0 };
  function Static({ label }) { runs.Static++; return h('text', {}, label); }
  function Shown({ value }) { runs.Shown++; return h('text', {}, String(value)); }
  const render = ({ data }) => h('view', {}, h(Static, { label: 'fixed' }), h(Shown, { value: data.n }));
  describe(render, view({ n: 1 }));
  describe(render, view({ n: 2 }));
  assert.deepEqual(runs, { Static: 1, Shown: 2 });
  assert.deepEqual(texts(describe(render, view({ n: 3 }))), ['fixed', '3']);
  assert.deepEqual(runs, { Static: 1, Shown: 3 });
  describe(render, view({ n: 3 }, { theme: 'dark' }));
  assert.deepEqual(runs, { Static: 1, Shown: 3 }, 'an environment field nothing read re-runs nothing');
});

test('a recalled component keeps its state, and re-runs when a cell it declared or read changes', () => {
  let own, outer, runs = 0;
  function Counter({ shared }) { runs++; own = useLocal('n', 0); return h('text', {}, `${own.value}/${shared.value}`); }
  const render = () => { outer = useLocal('outer', 'a'); return h('view', {}, h(Counter, { shared: outer })); };
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
  const render = () => h('view', {}, h(Reader), h(Clock));
  describe(render, view({ n: 1 }));
  assert.deepEqual(texts(describe(render, view({ n: 2 }))), ['2', 'now']);
  assert.deepEqual([reads, volatile], [2, 2]);
});

test('a component reading only useEnv() is kept across data changes and re-runs on a new environment', () => {
  let runs = 0, seen;
  function Theme() { runs++; seen = useEnv(); return h('text', {}, seen.theme); }
  const render = ({ data }) => h('view', {}, h(Theme), h('text', {}, String(data.n)));
  describe(render, view({ n: 1 }, { theme: 'light' }));
  assert.deepEqual(texts(describe(render, view({ n: 2 }, { theme: 'light' }))), ['light', '2']);
  assert.equal(runs, 1, 'a data change leaves it');
  assert.deepEqual(texts(describe(render, view({ n: 2 }, { theme: 'dark' }))), ['dark', '2']);
  assert.equal(runs, 2);
  assert.ok(Object.isFrozen(seen));
  assert.throws(() => useEnv(), /UI_SCOPE/);
});

test('a useEnv() reader re-runs only when a field it read changes', () => {
  const runs = { Theme: 0, Spread: 0 };
  function Theme() { runs.Theme++; return h('text', {}, useEnv().theme ?? 'light'); }
  function Spread() { runs.Spread++; return h('text', {}, String(Object.keys({ ...useEnv() }).length)); }
  const render = () => h('view', {}, h(Theme), h(Spread));
  describe(render, view({}, { theme: 'light', keyboard: { height: 0 } }));
  describe(render, view({}, { theme: 'light', keyboard: { height: 300 } }));
  assert.deepEqual(runs, { Theme: 1, Spread: 2 }, 'a keyboard change leaves the theme reader; a spread read every field');
  assert.equal(texts(describe(render, view({}, { theme: 'dark', keyboard: { height: 300 } })))[0], 'dark');
  assert.deepEqual(runs, { Theme: 2, Spread: 3 });
});

test('a component re-runs when an environment field its classes lower against changes', () => {
  configureClasses([
    { name: 'card', when: {}, style: { backgroundColor: 'surface' } },
    { name: 'dark:card-dark', when: { theme: 'dark' }, style: { borderWidth: 1 } },
    { name: 'md:wide', when: { minWidth: 768 }, style: { paddingTop: 8 } },
    { name: 'half', when: {}, style: { height: '50vh' } },
  ]);
  const runs = { Plain: 0, Themed: 0, Wide: 0, Half: 0 };
  const part = (name, className) => function () { runs[name]++; return h('view', { className, 'data-testid': name }); };
  const Plain = part('Plain', 'card'), Themed = part('Themed', 'card dark:card-dark'), Wide = part('Wide', 'md:wide'), Half = part('Half', 'half');
  const render = () => h('view', {}, h(Plain), h(Themed), h(Wide), h(Half));
  const env = { theme: 'light', viewport: { width: 400, height: 800 }, insets: { top: 0, right: 0, bottom: 0, left: 0 }, keyboard: { height: 0 } };
  const node = (buffer, id) => decode(buffer).find(candidate => candidate.testId === id);
  describe(render, view({}, env));
  const dark = describe(render, view({}, { ...env, theme: 'dark' }));
  assert.deepEqual(runs, { Plain: 1, Themed: 2, Wide: 1, Half: 1 });
  assert.equal(node(dark, 'Themed').props.borderTopWidth, 1);
  const keyboard = describe(render, view({}, { ...env, theme: 'dark', keyboard: { height: 400 } }));
  assert.deepEqual(runs, { Plain: 1, Themed: 2, Wide: 1, Half: 2 }, 'a vh length lowers against the keyboard');
  assert.equal(node(keyboard, 'Half').props.height, 200);
  describe(render, view({}, { ...env, theme: 'dark', keyboard: { height: 400 }, viewport: { width: 900, height: 800 } }));
  assert.deepEqual(runs, { Plain: 1, Themed: 2, Wide: 2, Half: 3 });
});

test('callbacks in a recalled subtree still dispatch, and a discarded render keeps the committed one', () => {
  let clicks = 0, runs = 0;
  const onClick = () => { clicks++; };
  function Button({ label }) { runs++; return h('button', { onClick }, label); }
  const render = ({ data }) => h('view', {}, h(Button, { label: 'Go' }), h('text', {}, String(data.n)));
  describe(render, view({ n: 1 }));
  prepare(render, view({ n: 2 })); discard();
  const output = decode(describe(render, view({ n: 3 })));
  assert.equal(runs, 1);
  assert.ok(dispatch(output.find(node => node.tag === 'button').props.onClick));
  assert.equal(clicks, 1);
});

test('equal JSX children and plain props compare by value', () => {
  let runs = 0;
  function Card({ style, children }) { runs++; return h('view', { style }, children); }
  const render = ({ data }) => h('view', {}, h(Card, { style: { padding: 4 } }, h('text', {}, 'body')), h('text', {}, String(data.n)));
  describe(render, view({ n: 1 }));
  describe(render, view({ n: 2 }));
  assert.equal(runs, 1);
  const changed = ({ data }) => h('view', {}, h(Card, { style: { padding: 4 } }, h('text', {}, `body ${data.n}`)));
  describe(changed, view({ n: 3 }));
  assert.equal(runs, 2);
});

test('a recalled list keeps rows that hold state rendering against their cells', () => {
  const items = Object.freeze(['a', 'b']), cells = new Map();
  const renderItem = item => { const cell = useLocal('on', false); cells.set(item, cell); return h('text', {}, `${item}:${cell.value}`); };
  function Rows() { return h(List, { items, getKey: String, renderItem }); }
  const render = ({ data }) => h('view', {}, h(Rows), h('text', {}, String(data.n)));
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
