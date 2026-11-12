import { test, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { decode } from './support/wire.mjs';
import { h, listTag, describe, prepare, commit, dispatch, reset, useLocal, createView } from '../src/internal.js';
import { createContext, useContext } from '../src/index.js';

beforeEach(() => reset());

const texts = buffer => decode(buffer).filter(node => node.tag === 'text').map(node => node.text);
const all = { root: { start: 0, end: 3, leading: 0, trailing: 0 } };

test('a reader sees the nearest provider, and the default with none above it', () => {
  const Theme = createContext('light');
  function Label() { return h('text', {}, useContext(Theme)); }
  const render = () => h('view', {},
    h(Label),
    h(Theme.Provider, { value: 'dark' }, h('view', {}, h(Label),
      h(Theme.Provider, { value: 'dim' }, h(Label)))));
  assert.deepEqual(texts(describe(render, createView({}, {}))), ['light', 'dark', 'dim']);
});

test('a Provider wraps one element and holds plain values; useContext belongs to render', () => {
  const Ctx = createContext(0);
  const throws = (render, pattern) => assert.throws(() => describe(render, createView({}, {})), pattern);
  throws(() => h(Ctx.Provider, { value: 1 }, h('view'), h('view')), /UI_CONTEXT: a Provider wraps one element/);
  throws(() => h(Ctx.Provider, { value: 1 }, 'text'), /UI_CONTEXT: a Provider wraps one element/);
  function Pair() { return [h('view', { key: 'a' }), h('view', { key: 'b' })]; }
  throws(() => h(Ctx.Provider, { value: 1 }, h(Pair)), /UI_CONTEXT: a Provider wraps one element/);
  throws(() => h(Ctx.Provider, {}, h('view')), /UI_CONTEXT: a Provider needs a value/);
  throws(() => h(Ctx.Provider, { value: 1, className: 'p-1' }, h('view')), /UI_CONTEXT: a Provider takes only value/);
  throws(() => h(Ctx.Provider, { value: new Map() }, h('view')), /UI_CONTEXT: value must be plain data/);
  throws(() => h(Ctx.Provider, { value: { at: new Date() } }, h('view')), /UI_CONTEXT: value must be plain data/);
  assert.throws(() => createContext(), /UI_CONTEXT: createContext takes a default value/);
  assert.throws(() => useContext(Ctx), /UI_SCOPE/);
  throws(() => { createContext(1); return h('view'); }, /UI_CONTEXT: createContext belongs at module top level/);
  throws(() => { useContext({ Provider: {} }); return h('view'); }, /UI_CONTEXT: expected a context/);
  // A Local binding is a handle: providing it reads nothing.
  let ran = 0;
  function Reader() { ran++; return h('text', {}, String(useContext(Ctx).value)); }
  const render = () => { const n = useLocal('n', 4); return h(Ctx.Provider, { value: n }, h(Reader)); };
  assert.deepEqual(texts(describe(render, createView({}, {}))), ['4']);
  assert.equal(ran, 1);
});

test('a changed value re-runs its readers and what encloses them, not siblings that did not read it', () => {
  const Mode = createContext('a');
  const runs = { Reader: 0, Wrapper: 0, Other: 0 };
  function Reader() { runs.Reader++; return h('text', {}, useContext(Mode).mode); }
  function Wrapper() { runs.Wrapper++; return h('view', {}, h(Reader)); }
  function Other() { runs.Other++; return h('text', {}, 'other'); }
  let mode;
  const render = () => { mode = useLocal('mode', 'a');
    return h(Mode.Provider, { value: { mode: mode.value } }, h('view', {}, h(Wrapper), h(Other))); };
  const run = () => { const out = texts(prepare(render, createView({}, {}), all)); commit(); return out; };
  assert.deepEqual(run(), ['a', 'other']);
  const base = { ...runs };
  run();
  assert.deepEqual(runs, base, 'an equal object literal re-runs nothing');
  mode.set('b');
  assert.deepEqual(texts(prepare(render, createView({}, {}), all)), ['b', 'other']); commit();
  assert.deepEqual(runs, { Reader: base.Reader + 1, Wrapper: base.Wrapper + 1, Other: base.Other });
});

test('a reader under a provider inside a recalled component still matches', () => {
  const Mode = createContext('none');
  let runs = 0;
  function Reader() { runs++; return h('text', {}, useContext(Mode)); }
  function Section({ label }) { return h(Mode.Provider, { value: label }, h('view', {}, h(Reader))); }
  let outer;
  const render = () => { outer = useLocal('outer', 0); return h('view', {}, h('text', {}, String(outer.value)), h(Section, { label: 'x' })); };
  prepare(render, createView({}, {}), all); commit();
  outer.set(1);
  assert.deepEqual(texts(prepare(render, createView({}, {}), all)), ['1', 'x']); commit();
  assert.equal(runs, 1, 'the provider sits inside Section, so its readers stay recallable');
});

test('List rows read the providers above the list and re-render when they change', () => {
  const Label = createContext('?');
  let runs = 0, prefix;
  function Cell({ i }) { runs++; return h('text', {}, `${useContext(Label)}${i}`); }
  // The list sits in a component that does not read the context, so only the chain check re-runs it.
  function Rows() { return h(listTag, { count: 1, row: i => h(Cell, { i }) }); }
  function Factory() { return h(listTag, { count: 1, row: () => h('text', {}, `f:${useContext(Label)}`) }); }
  const render = () => { prefix = useLocal('prefix', 'a');
    return h(Label.Provider, { value: prefix.value }, h('view', {}, h(Rows), h(Factory))); };
  assert.deepEqual(texts(prepare(render, createView({}, {}), all)), ['a0', 'f:a']); commit();
  prefix.set('b');
  assert.deepEqual(texts(prepare(render, createView({}, {}), all)), ['b0', 'f:b']); commit();
  assert.equal(runs, 2);
});

test('a reader keeps matching after its context module is swapped for a new object with the same id', () => {
  const before = createContext('default'); before.__lxid = 'pages/kit#Tabs';
  const after = createContext('default'); after.__lxid = 'pages/kit#Tabs';
  function Reader() { return h('text', {}, useContext(after)); }
  assert.deepEqual(texts(describe(() => h(before.Provider, { value: 'provided' }, h(Reader)), createView({}, {}))), ['provided']);
  // Swapping keeps the Provider's scope, so state under it survives.
  let count;
  function Counter() { count = useLocal('n', 0); return h('button', { onClick: () => count.set(count.value + 1) }, String(count.value)); }
  const page = context => () => h(context.Provider, { value: 'v' }, h(Counter));
  const buttons = buffer => decode(buffer).filter(node => node.tag === 'button');
  const [button] = buttons(prepare(page(before), createView({}, {}), all)); commit();
  dispatch(button.props.onClick);
  assert.equal(buttons(prepare(page(after), createView({}, {}), all))[0].text, '1');
});
