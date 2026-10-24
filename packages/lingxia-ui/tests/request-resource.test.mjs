import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, prepareWindows, commit, discard, describe, reset, configureActions, createView, useRequest, takeActionRequests, receiveActionResponse, hasLocalUpdates } from '../src/runtime.js';
const frames = () => takeActionRequests().map(JSON.parse);
const flush = async () => { await Promise.resolve(); await Promise.resolve(); };
const setup = () => { reset(); configureActions({ __names: ['probe', 'ticks'], __modes: { probe: 'call', ticks: 'stream' } }); return createView({}, {}); };
const answer = (id, value) => receiveActionResponse({ v: 2, kind: 'res', id, ok: true, result: value });

test('requests preserve omitted payloads and distinguish explicit null across reloads', async () => {
  const view = setup(); let resource, params;
  const render = () => { resource = useRequest('load', view.actions.probe, params); return h('text'); };
  for (const value of [undefined, null, {id:1}, undefined]) {
    params = value;
    describe(render, view);
    const [frame] = frames();
    assert.equal(Object.hasOwn(frame, 'params'), value !== undefined);
    assert.deepEqual(frame.params, value);
    answer(frame.id, 'ok'); await flush();
    describe(render, view); assert.deepEqual(frames(), []);
    const pending = resource.reload(); const [retry] = frames();
    assert.equal(Object.hasOwn(retry, 'params'), value !== undefined);
    assert.deepEqual(retry.params, value);
    answer(retry.id, 'ok'); await pending;
  }
});

test('action references wait for the catalog and reject stale or arbitrary functions', async () => {
  reset(); let request;
  const render = view => { request = useRequest('record', view.actions.probe, {id:1}); return h('text'); };
  describe(render, createView({}, {})); assert.deepEqual(frames(), []);
  configureActions({__names:['probe'],__modes:{probe:'call'}});
  const view = createView({}, {});
  describe(render, view);
  const [frame] = frames(); assert.equal(frame.method,'probe'); assert.deepEqual(frame.params,{id:1});
  answer(frame.id, {title:'Typed'}); await flush(); assert.equal(request.value.title,'Typed');
  const stale = view.actions.probe, next = setup();
  assert.throws(() => describe(() => { useRequest('record', stale); return h('text'); }, next), /UI_REQUEST/);
  assert.throws(() => describe(() => { useRequest('record', async () => 1); return h('text'); }, next), /UI_REQUEST/);
});

test('request starts after commit only and survives refinement without duplication', async () => {
  const view = setup(); let request;
  const render = () => { request = useRequest('bootstrap', 'probe', { n: 1 }); return h('text'); };
  prepare(render, view); discard(); assert.deepEqual(frames(), []);
  prepare(render, view); prepareWindows({}); commit();
  const [frame] = frames(); assert.equal(frame.method, 'probe'); assert.equal(request.pending, true);
  answer(frame.id, { n: 1 }); await flush();
  assert.deepEqual(request.value, { n: 1 });
  describe(render, view); assert.deepEqual(frames(), []); assert.equal(hasLocalUpdates(), false);
});

test('replacement ignores out-of-order success and failure; discarded params do not run', async () => {
  const view = setup(); let request;
  const render = n => () => { request = useRequest('probe', 'probe', { n }); return h('text'); };
  describe(render(1), view); const [old] = frames();
  prepare(render(2), view); discard(); assert.deepEqual(frames(), []);
  describe(render(3), view); const [current] = frames();
  answer(current.id, 3); await flush(); answer(old.id, 1); await flush();
  assert.equal(request.value, 3);
  const reload = request.reload(); const [retry] = frames();
  receiveActionResponse({ v: 2, kind: 'res', id: retry.id, ok: false, error: { code: 'FAILED', message: 'offline' } }); await reload;
  assert.equal(request.error, 'offline'); assert.equal(request.pending, false);
});

test('removal and reset invalidate late results and retained reload handles', async () => {
  const view = setup(); let request;
  describe(() => { request = useRequest('probe', 'probe'); return h('text'); }, view);
  const [frame] = frames();
  describe(() => h('text'), view);
  answer(frame.id, 'late'); await flush();
  assert.equal(request.value, null); assert.equal(hasLocalUpdates(), false);
  await request.reload(); assert.deepEqual(frames(), []);
  describe(() => { request = useRequest('probe', 'probe'); return h('text'); }, view);
  reset(); await flush(); assert.equal(hasLocalUpdates(), false);
});

test('catalog arrival starts a waiting request and catalog replacement retries', async () => {
  reset(); let request;
  const render = () => { request = useRequest('probe', 'probe'); return h('text'); };
  describe(render, createView({}, {})); assert.deepEqual(frames(), []);
  configureActions({ __names: ['probe'], __modes: { probe: 'call' } });
  describe(render, createView({}, {}));
  const [old] = frames();
  configureActions({ __names: ['probe'], __modes: { probe: 'call' } });
  describe(render, createView({}, {})); const [fresh] = frames();
  await flush(); answer(fresh.id, 'fresh'); await flush();
  assert.equal(request.value, 'fresh'); assert.notEqual(old.id, fresh.id);
});

test('streams, duplicate names and render-time reload are rejected without issuing requests', () => {
  const view = setup();
  for (const render of [
    () => { useRequest('r', 'host.call'); return h('text'); },
    () => { useRequest('r', 'missing'); return h('text'); },
    () => { useRequest('r', 'ticks'); return h('text'); },
    () => { useRequest('r', 'probe'); useRequest('r', 'probe'); return h('text'); },
    () => { useRequest('r', 'probe').reload(); return h('text'); },
  ]) { assert.throws(() => prepare(render, view), /UI_REQUEST|UI_RENDER/); discard(); assert.deepEqual(frames(), []); }
});
