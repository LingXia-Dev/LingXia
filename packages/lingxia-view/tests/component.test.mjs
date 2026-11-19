import { test, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { Fragment } from '../src/runtime/nodes.js';
import { rendered } from './support/wire.mjs';
import { audit } from '../compiler/index.mjs';
import { h, List, childrenToArray, elementType, cloneElement, describe, prepare, discard, reset, dispatch, useLocal, useRequest, configureActions, createView, takeActionRequests } from '../src/internal.js';

beforeEach(() => reset());

test('data List keeps item state through reorder and rejects duplicate keys', () => {
  let items = [{id:'a'}, {id:'b'}]; const cells = new Map();
  const render = () => h(List, {items, getKey:item => item.id, renderItem:item => {
    const cell = useLocal('value', item.id); cells.set(item.id, cell);
    return h('text', {}, cell.value);
  }});
  describe(render, {}); cells.get('a').set('changed'); describe(render, {});
  items.reverse(); const output = describe(render, {});
  assert.ok(output.includes('b'));
  items.reverse(); assert.ok(describe(render, {}).includes('changed'));
  items = [{id:'a'}, {id:'a'}]; assert.throws(() => describe(render, {}), /UI_LIST/);
});

test('component children preserve zero/one/many and explicit JSX precedence', () => {
  const seen = [];
  function Forward({children}) { seen.push(children); return children ?? h('text', {}, 'empty'); }
  const records = describe(() => h('view', {}, h(Forward), h(Forward, {children:h('text', {}, 'ignored')}, h('text', {}, 'one'))), {});
  assert.equal(seen[0], undefined);
  assert.equal(Array.isArray(seen[1]), false);
  assert.ok(records.includes('one'));
  assert.ok(!records.includes('ignored'));
  function Many({children}) { assert.equal(children.length, 2); return h('view', {}, children); }
  assert.ok(describe(() => h(Many, {}, h('text', {}, 'a'), h('text', {}, 'b')), {}).includes('b'));
  assert.deepEqual(childrenToArray([null, ['a', false, [1]], undefined]), ['a', 1]);
});

test('elementType names an intrinsic, a component or a fragment, and nothing else', () => {
  function Button() { return h('text', {}, 'b'); }
  const seen = [];
  describe(() => {
    seen.push(elementType(h('view')), elementType(h(Button)), elementType(h(Fragment, {}, 'x')),
      elementType('text'), elementType(null), elementType(false), elementType([h('view')]));
    return h('text', {}, 'x');
  }, {});
  assert.deepEqual(seen, ['view', Button, Fragment, undefined, undefined, undefined, undefined]);
});

test('a stale element handle cannot be read or cloned', () => {
  let kept;
  describe(() => (kept = h('text', {}, 'a')), {});
  describe(() => {
    assert.throws(() => elementType(kept), /UI_STALE/);
    assert.throws(() => cloneElement(kept, {}), /UI_STALE/);
    return h('text', {}, 'b');
  }, {});
});

test('a clone joins className, replaces other props and keeps local state in its place', () => {
  const props = [], cells = [];
  function Trigger({ className, label }) {
    const cell = useLocal('count', 0); cells.push(cell); props.push({ className, label });
    return h('text', {}, `${label} ${cell.value}`);
  }
  function Wrap({ clone, children }) {
    const [child] = childrenToArray(children);
    return clone ? cloneElement(child, { className: 'added', label: 'cloned' }) : child;
  }
  let clone = false;
  const render = () => h(Wrap, { clone }, h(Trigger, { className: 'own', label: 'plain' }));
  assert.ok(describe(render, {}).includes('plain 0'));
  cells.at(-1).set(5);
  clone = true;
  assert.ok(describe(render, {}).includes('cloned 5'), 'the clone keeps the original\'s local state');
  assert.deepEqual(props.at(-1), { className: 'own added', label: 'cloned' });
});

test('cloneElement refuses replacing a handler unless it is named in override', () => {
  const own = () => {}, added = () => {};
  let got;
  function Trigger({ onClick, onFocus }) { got = { onClick, onFocus }; return h('text', {}, 't'); }
  const render = options => () => cloneElement(h(Trigger, { onClick: own }), { onClick: added, onFocus: added }, options);
  assert.throws(() => describe(render(), {}), /UI_PROP: cloneElement would replace onClick on <Trigger>; pass \{ override: \["onClick"\] \} to replace it/);
  describe(render({ override: ['onClick'] }), {});
  assert.equal(got.onClick, added);
  assert.equal(got.onFocus, added);
});

test('cloneElement takes one element and leaves key and children alone', () => {
  describe(() => {
    const child = h('text', { key: 'a' }, 'x');
    assert.throws(() => cloneElement('text', {}), /UI_PROP: cloneElement takes one element; use childrenToArray first/);
    assert.throws(() => cloneElement(null, {}), /UI_PROP: cloneElement takes one element/);
    assert.throws(() => cloneElement([child], {}), /UI_PROP: cloneElement takes one element/);
    assert.throws(() => cloneElement(child, { key: 'b' }), /UI_PROP: cloneElement does not change key or children/);
    assert.throws(() => cloneElement(child, { children: 'y' }), /UI_PROP: cloneElement does not change key or children/);
    return cloneElement(child, { 'data-testid': 'kept' });
  }, {});
});

test('duplicate locals fail with component context, while custom hook namespaces stay independent', () => {
  const useDraft = name => useLocal(`${name}/value`, '');
  function Bad() { useDraft('draft'); useDraft('draft'); return h('text'); }
  assert.throws(() => describe(() => h(Bad), {}), /UI_LOCAL: duplicate declaration.*draft\/value[\s\S]*at Bad/);
  let first, second;
  const render = () => { first = useDraft('first'); second = useDraft('second'); return h('text', {}, first.value + second.value); };
  describe(render, {}); first.set('a'); second.set('b');
  assert.ok(describe(render, {}).includes('ab'));
  first.set('discarded'); prepare(render, {}); discard();
  assert.ok(describe(render, {}).includes('ab'));
});

test('a declined candidate keeps pending updates; a rejected one and reset drop them', () => {
  let count;
  const render = () => { count = useLocal('count', 0); return h('text', {}, `n ${count.value}`); };
  describe(render, {});
  count.set(1); prepare(render, {}); discard(true);
  assert.ok(describe(render, {}).includes('n 1'), 'the frame the target declined did not cost the update');
  count.set(2); prepare(render, {}); discard();
  assert.ok(describe(render, {}).includes('n 1'), 'native rejected the candidate: its update reverts');
  count.set(3); reset();
  assert.ok(describe(render, {}).includes('n 0'), 'reset carries no pending update');
});

test('empty invoked components retain own state and dispose descendants only after commit', () => {
  let visible = true, owner, child;
  function Child() { child = useLocal('value', 'child'); return h('text', {}, child.value); }
  function Optional({ visible }) { owner = useLocal('value', 'owner'); return visible ? h(Child) : null; }
  const render = () => h('view', {}, h(Optional, { visible }));
  describe(render, {}); owner.set('kept'); child.set('changed'); describe(render, {});
  visible = false; prepare(render, {}); discard(); visible = true;
  assert.ok(describe(render, {}).includes('changed'));
  visible = false; describe(render, {}); assert.equal(owner.value, 'kept');
  assert.throws(() => child.set('stale'), /UI_LOCAL: "value" in .* is no longer mounted/); visible = true;
  assert.ok(describe(render, {}).includes('child')); assert.equal(owner.value, 'kept');
  for (const value of [null, undefined, false, true]) {
    assert.doesNotThrow(() => describe(() => h('view', {}, h(() => value)), {}));
    assert.throws(() => describe(() => h(() => value), {}), /UI_ROOT/);
  }
});

test('an empty component keeps its request while removing its descendant request', () => {
  configureActions({__names:['load'], __modes:{load:'call'}});
  const view = createView({}, {}); let visible = true;
  function Child() { useRequest('load', view.actions.load, {owner:'child'}); return h('text'); }
  function Optional({ visible }) { useRequest('load', view.actions.load, {owner:'parent'}); return visible ? h(Child) : null; }
  const render = () => h('view', {}, h(Optional, { visible }));
  describe(render, view); const started = takeActionRequests().map(JSON.parse);
  assert.equal(started.length, 2);
  visible = false; describe(render, view); takeActionRequests();
  visible = true; describe(render, view);
  const restarted = takeActionRequests().map(JSON.parse);
  assert.equal(restarted.length, 1); assert.equal(restarted[0].params.owner, 'child');
});

test('local JSON domain omits optional object fields without admitting invalid array values', () => {
  let local;
  const render = () => { local = useLocal('draft', {title:'one', note:undefined, nested:{missing:undefined}}); return h('text'); };
  describe(render, {}); assert.deepEqual(local.value, {title:'one', nested:{}});
  local.set({title:'two', note:undefined}); describe(render, {});
  assert.deepEqual(local.value, {title:'two'}); assert.ok(Object.isFrozen(local.value));
  for (const initial of [undefined, [undefined], new Date(), {callback() {}}, Infinity])
    assert.throws(() => describe(() => { useLocal('invalid', initial); return h('text'); }, {}), /UI_LOCAL/);
});

test('fragments lay their children out in their place, keeping each child slot', () => {
  const text = (value, props = {}) => h('text', props, value);
  const inline = rendered(() => h('view', {}, text('a'), h(Fragment, null, false, text('b'), text('c')), text('d')));
  assert.deepEqual(inline.children.map(child => [child.text, child.props.childSlot]), [['a', undefined], ['b', '1.1'], ['c', '1.2'], ['d', '2']]);
  function Pair() { return h(Fragment, null, text('x'), text('y')); }
  const component = rendered(() => h('view', {}, false, h(Pair)));
  assert.deepEqual(component.children.map(child => [child.text, child.props.childSlot]), [['x', '1.0'], ['y', '1.1']]);
  // A keyed group moves together: its children are named by the group key.
  const groups = order => rendered(() => h('view', {}, order.map(id => h(Fragment, { key: id }, text(`${id}1`), text(`${id}2`, { key: 'tail' })))));
  const keys = groups(['a', 'b']).children.map(child => [child.text, child.key]);
  assert.deepEqual(keys, [['a1', '["a","#0"]'], ['a2', '["a","tail"]'], ['b1', '["b","#0"]'], ['b2', '["b","tail"]']]);
  assert.deepEqual(groups(['b', 'a']).children.map(child => child.key), [keys[2][1], keys[3][1], keys[0][1], keys[1][1]]);
  function Keyed() { return h(Fragment, null, text('k', { key: 'same' })); }
  assert.throws(() => describe(() => h('view', {}, h(Keyed), h(Keyed)), {}), /UI_KEY: duplicate sibling/);
  assert.throws(() => describe(() => h(Fragment, null, text('a'), text('b')), {}), /UI_ROOT/);
  assert.throws(() => describe(() => h('text', {}, h(Pair)), {}), /UI_TEXT: a component inside <text> returns a <text> beside its text; with text it may return <span>, <link>, a popover or a <dialog>/);
});

test('local state inside a fragment child survives a sibling appearing before the group', () => {
  let cell;
  function Counter() { cell = useLocal('n', 0); return h('text', {}, String(cell.value)); }
  let extra = false;
  const render = () => h('view', {}, extra && h('text', {}, 'new'), h(Fragment, null, h(Counter)));
  describe(render, {}); cell.set(5); describe(render, {});
  extra = true;
  assert.ok(rendered(render).children.some(child => child.text === '5'));
});

test('useLocal(initial) keys state by call order, independent of named locals', () => {
  const cells = [];
  function Counter() {
    const a = useLocal(0), named = useLocal('label', 'x'), b = useLocal('');
    cells.splice(0, 3, a, named, b);
    return h('text', {}, `${a.value}|${named.value}|${b.value}`);
  }
  const render = () => h('view', {}, h(Counter, { key: 'one' }), h(Counter, { key: 'two' }));
  describe(render, {});
  cells[0].set(5); cells[2].set('typed');
  const output = describe(render, {});
  assert.ok(output.includes('5|x|typed'));
  assert.throws(() => describe(() => { useLocal(); return h('text'); }, {}), /UI_LOCAL: useLocal takes \(initial\) or \(name, initial\)/);
});

test('details, summary and progress are not elements; the build names the primitives that replace them', () => {
  assert.deepEqual(audit('export function render() { return <page><details/><progress/></page>; }', 'page.tsx'), [
    'page.tsx:1:41: UI_TAG: unsupported tag <details>; use <button aria-expanded> and a body shown from useLocal state',
    'page.tsx:1:51: UI_TAG: unsupported tag <progress>; use <view role="progressbar" aria-valuenow>',
  ]);
});
