import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, prepareWindows, commit, discard, describe, reset, configureActions, createView, useStream, takeActionRequests, receiveActionResponse, hasLocalUpdates } from '../src/runtime.js';
const frames = () => takeActionRequests().map(JSON.parse);
const flush = async () => { await Promise.resolve(); await Promise.resolve(); };
const setup = () => { reset(); configureActions({ __names: ['ticks', 'echo'], __modes: { ticks: 'stream', echo: 'call' } }); return createView({}, {}); };
const event = (id, seq, payload) => receiveActionResponse({ v: 2, kind: 'event', id, seq, payload });
const end = (id, result) => receiveActionResponse({ v: 2, kind: 'res', id, ok: true, result });

test('stream action references preserve commit ownership and typed payload transport', async () => {
  reset(); let stream; const values = [];
  const render = view => { stream = useStream('watch', view.actions.ticks, {onData:value=>values.push(value)}); return h('text'); };
  describe(render, createView({}, {})); assert.equal(stream.status,'idle');
  configureActions({__names:['ticks'],__modes:{ticks:'stream'}});
  const view = createView({}, {});
  prepare(render, view); discard(); stream.start({count:1}); assert.deepEqual(frames(),[]);
  describe(render, view); stream.start({count:2});
  const [frame] = frames(); assert.equal(frame.method,'ticks'); assert.deepEqual(frame.params,{count:2});
  event(frame.id,0,{token:'hello'}); end(frame.id,2); await flush();
  assert.deepEqual(values,[{token:'hello'}]); assert.equal(stream.result,2);
  const stale = view.actions.ticks, next = setup();
  assert.throws(() => describe(() => { useStream('watch',stale,{onData(){}}); return h('text'); },next),/UI_STREAM/);
});

test('manual stream starts only after commit, retains callbacks across redraw and returns its result', async () => {
  const view = setup(), seen = []; let stream;
  const render = label => () => { stream = useStream('ticks', 'ticks', { onData: value => seen.push([label, value]) }); return h('text'); };
  prepare(render('abandoned'), view); discard(); stream.start(); assert.deepEqual(frames(), []);
  describe(render('first'), view); assert.deepEqual(frames(), []);
  stream.start({ count: 2 }); const [request] = frames();
  event(request.id, 0, 1);
  prepare(render('discarded'), view); discard(); event(request.id, 1, 2);
  describe(render('next'), view); prepareWindows({}); commit(); event(request.id, 2, 3);
  assert.deepEqual(seen, [['first', 1], ['first', 2], ['next', 3]]);
  assert.deepEqual(frames(), []);
  end(request.id, 'done'); await flush();
  assert.equal(stream.status, 'completed'); assert.equal(stream.result, 'done');
});

test('restart cancels old stream and ignores its late events and rejection', async () => {
  const view = setup(), seen = []; let stream;
  describe(() => { stream = useStream('ticks', 'ticks', { onData: value => seen.push(value) }); return h('text'); }, view);
  stream.start(); const [old] = frames();
  stream.start(); const [cancel, next] = frames(); assert.equal(cancel.id, old.id); assert.equal(cancel.kind, 'cancel');
  await flush(); assert.equal(stream.status, 'streaming');
  assert.equal(event(old.id, 0, 'old'), false); event(next.id, 0, 'new'); assert.deepEqual(seen, ['new']);
  stream.cancel(); stream.cancel(); assert.equal(frames().length, 1);
  await flush(); assert.equal(stream.status, 'canceled'); assert.equal(stream.error, null);
});

test('scope removal cancels without local updates and stale start cannot reopen', async () => {
  const view = setup(); let stream;
  describe(() => { stream = useStream('ticks', 'ticks', { onData() {} }); return h('text'); }, view);
  stream.start(); frames();
  describe(() => h('text'), view); assert.equal(frames()[0].kind, 'cancel');
  await flush(); assert.equal(hasLocalUpdates(), false);
  stream.start(); assert.deepEqual(frames(), []);
});

test('callback failure stops its producer and catalog replacement invalidates ownership', async () => {
  const view = setup(); let stream;
  const render = () => { stream = useStream('ticks', 'ticks', { onData() { throw Error('bad payload'); } }); return h('text'); };
  describe(render, view); stream.start(); const [request] = frames(); event(request.id, 0, 1);
  await flush(); assert.equal(stream.status, 'failed'); assert.equal(stream.error, 'bad payload');
  assert.equal(frames()[0].kind, 'cancel');
  stream.start(); frames();
  configureActions({ __names: ['ticks'], __modes: { ticks: 'stream' } });
  describe(render, createView({}, {})); await flush();
  assert.equal(stream.status, 'idle'); assert.equal(stream.error, null);
  stream.start(); assert.equal(frames()[0].method, 'ticks');
  reset(); await flush(); assert.equal(hasLocalUpdates(), false);
});

test('invalid methods, duplicate resources and render mutations never issue requests', () => {
  const view = setup();
  for (const method of ['echo', 'host.ticks', 'missing']) {
    assert.throws(() => prepare(() => { useStream('s', method, { onData() {} }); return h('text'); }, view), /UI_STREAM/);
    discard();
  }
  assert.throws(() => prepare(() => { useStream('s', 'ticks', { onData() {} }).start(); return h('text'); }, view), /UI_RENDER/);
  discard();
  assert.throws(() => prepare(() => { useStream('s', 'ticks', { onData() {} }); useStream('s', 'ticks', { onData() {} }); return h('text'); }, view), /duplicate/);
  discard(); assert.deepEqual(frames(), []);
});
