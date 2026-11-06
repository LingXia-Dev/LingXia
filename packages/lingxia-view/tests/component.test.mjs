import { test, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { Fragment } from '../src/runtime/nodes.js';
import * as details from '../src/runtime/details.js';
import { rendered } from './support/wire.mjs';
import { h, List, childrenToArray, describe, prepare, discard, reset, dispatch, useLocal, useRequest, configureActions, createView, takeActionRequests } from '../src/internal.js';

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
  const records = describe(() => h('column', {}, h(Forward), h(Forward, {children:h('text', {}, 'ignored')}, h('text', {}, 'one'))), {});
  assert.equal(seen[0], undefined);
  assert.equal(Array.isArray(seen[1]), false);
  assert.ok(records.includes('one'));
  assert.ok(!records.includes('ignored'));
  function Many({children}) { assert.equal(children.length, 2); return h('column', {}, children); }
  assert.ok(describe(() => h(Many, {}, h('text', {}, 'a'), h('text', {}, 'b')), {}).includes('b'));
  assert.deepEqual(childrenToArray([null, ['a', false, [1]], undefined]), ['a', 1]);
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
  const render = () => h('column', {}, h(Optional, { visible }));
  describe(render, {}); owner.set('kept'); child.set('changed'); describe(render, {});
  visible = false; prepare(render, {}); discard(); visible = true;
  assert.ok(describe(render, {}).includes('changed'));
  visible = false; describe(render, {}); assert.equal(owner.value, 'kept');
  assert.throws(() => child.set('stale'), /UI_LOCAL: "value" in .* is no longer mounted/); visible = true;
  assert.ok(describe(render, {}).includes('child')); assert.equal(owner.value, 'kept');
  for (const value of [null, undefined, false, true]) {
    assert.doesNotThrow(() => describe(() => h('column', {}, h(() => value)), {}));
    assert.throws(() => describe(() => h(() => value), {}), /UI_ROOT/);
  }
});

test('an empty component keeps its request while removing its descendant request', () => {
  configureActions({__names:['load'], __modes:{load:'call'}});
  const view = createView({}, {}); let visible = true;
  function Child() { useRequest('load', view.actions.load, {owner:'child'}); return h('text'); }
  function Optional({ visible }) { useRequest('load', view.actions.load, {owner:'parent'}); return visible ? h(Child) : null; }
  const render = () => h('column', {}, h(Optional, { visible }));
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
  const inline = rendered(() => h('column', {}, text('a'), h(Fragment, null, false, text('b'), text('c')), text('d')));
  assert.deepEqual(inline.children.map(child => [child.text, child.props.childSlot]), [['a', undefined], ['b', '1.1'], ['c', '1.2'], ['d', '2']]);
  function Pair() { return h(Fragment, null, text('x'), text('y')); }
  const component = rendered(() => h('column', {}, false, h(Pair)));
  assert.deepEqual(component.children.map(child => [child.text, child.props.childSlot]), [['x', '1.0'], ['y', '1.1']]);
  // A keyed group moves together: its children are named by the group key.
  const groups = order => rendered(() => h('column', {}, order.map(id => h(Fragment, { key: id }, text(`${id}1`), text(`${id}2`, { key: 'tail' })))));
  const keys = groups(['a', 'b']).children.map(child => [child.text, child.key]);
  assert.deepEqual(keys, [['a1', '["a","#0"]'], ['a2', '["a","tail"]'], ['b1', '["b","#0"]'], ['b2', '["b","tail"]']]);
  assert.deepEqual(groups(['b', 'a']).children.map(child => child.key), [keys[2][1], keys[3][1], keys[0][1], keys[1][1]]);
  function Keyed() { return h(Fragment, null, text('k', { key: 'same' })); }
  assert.throws(() => describe(() => h('column', {}, h(Keyed), h(Keyed)), {}), /UI_KEY: duplicate sibling/);
  assert.throws(() => describe(() => h(Fragment, null, text('a'), text('b')), {}), /UI_ROOT/);
  assert.throws(() => describe(() => h('text', {}, h(Pair)), {}), /UI_TEXT: a component inside <text> returns a fragment/);
});

test('local state inside a fragment child survives a sibling appearing before the group', () => {
  let cell;
  function Counter() { cell = useLocal('n', 0); return h('text', {}, String(cell.value)); }
  let extra = false;
  const render = () => h('column', {}, extra && h('text', {}, 'new'), h(Fragment, null, h(Counter)));
  describe(render, {}); cell.set(5); describe(render, {});
  extra = true;
  assert.ok(rendered(render).children.some(child => child.text === '5'));
});

test('Details discloses its body behind a Summary toggle that announces expanded', () => {
  const { Details, Summary } = details;
  const view = () => h('column', {}, h(Details, { 'data-testid': 'lic' }, h(Summary, {}, 'Licenses'), h('text', {}, 'MIT')));
  let root = rendered(view);
  const summary = root.children[0].children[0];
  assert.equal(summary.testId, 'lic-summary');
  assert.equal(JSON.stringify(summary.props).includes('stateExpanded'), true);
  assert.equal(summary.props.stateExpanded, 0);
  assert.equal(root.children[0].children.length, 1, 'closed: no body');
  // Clicking the summary opens it.
  const click = summary.props.onClick;
  assert.ok(click);
  dispatch(click);
  root = rendered(view);
  assert.equal(root.children[0].children[0].props.stateExpanded, 1);
  assert.equal(root.children[0].children[1].children[0].text, 'MIT');
  // Controlled: `open` wins and `onToggle` hears the request.
  let asked;
  const controlled = rendered(() => h(Details, { open: true, onToggle: next => { asked = next; } }, h(Summary, {}, 'More'), h('text', {}, 'body')));
  assert.equal(controlled.children.length, 2);
  dispatch(controlled.children[0].props.onClick);
  assert.equal(asked, false);
});

test('useLocal(initial) keys state by call order, independent of named locals', () => {
  const cells = [];
  function Counter() {
    const a = useLocal(0), named = useLocal('label', 'x'), b = useLocal('');
    cells.splice(0, 3, a, named, b);
    return h('text', {}, `${a.value}|${named.value}|${b.value}`);
  }
  const render = () => h('column', {}, h(Counter, { key: 'one' }), h(Counter, { key: 'two' }));
  describe(render, {});
  cells[0].set(5); cells[2].set('typed');
  const output = describe(render, {});
  assert.ok(output.includes('5|x|typed'));
  assert.throws(() => describe(() => { useLocal(); return h('text'); }, {}), /UI_LOCAL: useLocal takes \(initial\) or \(name, initial\)/);
});
