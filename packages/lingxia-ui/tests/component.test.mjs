import { test, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { h, List, childrenToArray, describe, prepare, discard, reset, useLocal, useRequest, configureActions, createView, takeActionRequests } from '../src/runtime.js';

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
  assert.throws(() => describe(() => h(Bad), {}), /UI_LOCAL: duplicate declaration.*draft\/value[\s\S]*at component Bad/);
  let first, second;
  const render = () => { first = useDraft('first'); second = useDraft('second'); return h('text', {}, first.value + second.value); };
  describe(render, {}); first.set('a'); second.set('b');
  assert.ok(describe(render, {}).includes('ab'));
  first.set('discarded'); prepare(render, {}); discard();
  assert.ok(describe(render, {}).includes('ab'));
});

test('empty invoked components retain own state and dispose descendants only after commit', () => {
  let visible = true, owner, child;
  function Child() { child = useLocal('value', 'child'); return h('text', {}, child.value); }
  function Optional() { owner = useLocal('value', 'owner'); return visible ? h(Child) : null; }
  const render = () => h('column', {}, h(Optional));
  describe(render, {}); owner.set('kept'); child.set('changed'); describe(render, {});
  visible = false; prepare(render, {}); discard(); visible = true;
  assert.ok(describe(render, {}).includes('changed'));
  visible = false; describe(render, {}); assert.equal(owner.value, 'kept');
  child.set('stale'); visible = true;
  assert.ok(describe(render, {}).includes('child')); assert.equal(owner.value, 'kept');
  for (const value of [null, undefined, false, true]) {
    assert.doesNotThrow(() => describe(() => h('column', {}, h(() => value)), {}));
    assert.throws(() => describe(() => h(() => value), {}), /UI_ROOT/);
  }
});

test('an empty component keeps its request while removing its descendant request', () => {
  configureActions({__names:['load'], __modes:{load:'call'}});
  const view = createView({}, {}); let visible = true;
  function Child() { useRequest('load', 'load', {owner:'child'}); return h('text'); }
  function Optional() { useRequest('load', 'load', {owner:'parent'}); return visible ? h(Child) : null; }
  const render = () => h('column', {}, h(Optional));
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
