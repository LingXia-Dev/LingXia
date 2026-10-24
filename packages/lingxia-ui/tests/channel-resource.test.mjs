import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  h, prepare, prepareWindows, commit, discard, reset, describe, useChannel,
  useLocal, configureActions, createView, takeActionRequests, receiveActionResponse,
  hasLocalUpdates,
} from '../src/runtime.js';

const flush = async () => { await Promise.resolve(); await Promise.resolve(); };
const requests = () => takeActionRequests().map(JSON.parse);
function setup() {
  reset();
  configureActions({ __names: ['ticker'], __modes: { ticker: 'call' } });
  return createView({}, {});
}
function ack(id, extra = {}) { receiveActionResponse({ v: 2, kind: 'ch.ack', id, ok: true, ...extra }); }

test('resource opens only after commit and survives redraw/window refinement without reopening', async () => {
  const view = setup(); let session, latest;
  const render = () => {
    latest = useLocal('last', '');
    session = useChannel('feed', 'ticker', { onData: value => latest.set(value) });
    return h('text', {}, latest.value);
  };
  prepare(render, view); assert.deepEqual(requests(), []); discard();
  prepare(render, view); prepareWindows({}); commit();
  const [open] = requests(); assert.equal(open.kind, 'ch.open');
  assert.equal(Object.hasOwn(open, 'params'), false, 'omitted channel params stay omitted');
  assert.equal(hasLocalUpdates(), true);
  ack(open.id); await flush(); assert.equal(session.connected, true);
  receiveActionResponse({ v: 2, kind: 'ch.data', id: open.id, seq: 0, payload: 'tick' });
  assert.equal(latest.value, 'tick');
  describe(render, view); prepareWindows({}); commit();
  assert.deepEqual(requests(), []); assert.equal(hasLocalUpdates(), false);
  describe(() => h('text', {}, 'gone'), view);
  assert.equal(requests()[0].kind, 'ch.close');
  assert.throws(() => session.send('stale'), /disconnected/);
});

test('late acknowledgement after removal closes instead of invoking old callbacks', async () => {
  const view = setup(); let calls = 0;
  describe(() => { useChannel('feed', 'ticker', { onData() { calls++; } }); return h('text'); }, view);
  const [open] = requests();
  describe(() => h('text'), view);
  ack(open.id); await flush();
  assert.equal(requests()[0].kind, 'ch.close');
  assert.equal(receiveActionResponse({ v: 2, kind: 'ch.data', id: open.id, seq: 0, payload: 1 }), false);
  assert.equal(calls, 0);
});

test('rejected automatic open reports state, manual reconnect works and close stays closed across redraw', async () => {
  const view = setup(); let session;
  const render = () => { session = useChannel('feed', 'ticker', { onData() {} }); return h('text'); };
  describe(render, view); const [open] = requests();
  ack(open.id, { ok: false, error: { code: 'FAILED', message: 'offline' } }); await flush();
  assert.equal(session.error, 'offline'); assert.equal(session.connecting, false);
  const connected = session.connect(); const [retry] = requests(); ack(retry.id); await connected;
  assert.equal(session.connected, true); assert.equal(session.error, null);
  session.close(); assert.equal(requests().length, 1);
  describe(render, view); assert.deepEqual(requests(), []);
  assert.equal(session.connected, false);
});

test('parameter replacement only takes effect on commit and old close cannot disconnect new resource', async () => {
  const view = setup(); let session;
  const render = symbol => () => {
    session = useChannel('feed', 'ticker', { params: { symbol }, onData() {} });
    return h('text');
  };
  describe(render('A'), view); const [first] = requests(); ack(first.id); await flush();
  prepare(render('B'), view); discard(); assert.deepEqual(requests(), []); assert.equal(session.connected, true);
  describe(render('B'), view); const frames = requests();
  assert.deepEqual(frames.map(x => x.kind), ['ch.close', 'ch.open']);
  ack(frames[1].id); await flush(); assert.equal(session.connected, true);
  reset(); await flush();
  assert.throws(() => session.send('stale'), /disconnected/);
});

test('resource names are scoped and render-time connection mutation is forbidden', () => {
  const view = setup();
  const Row = () => { useChannel('feed', 'ticker', { onData() {} }); return h('text'); };
  describe(() => h('column', {}, h(Row, { key: 'a' }), h(Row, { key: 'b' })), view);
  assert.equal(requests().length, 2);
  assert.throws(() => describe(() => {
    useChannel('same', 'ticker', { onData() {} });
    useChannel('same', 'ticker', { onData() {} });
    return h('text');
  }, view), /duplicate/);
  for (const method of ['send', 'close', 'connect']) {
    assert.throws(() => describe(() => {
      const session = useChannel('feed', 'ticker', { onData() {} }); session[method]();
      return h('text');
    }, view), /during render/);
  }
  assert.deepEqual(requests(), []);
});

test('list refinement retains resources with row state and shrinking closes removed rows', async () => {
  const view = setup(); let count = 3;
  const window = (start, end) => ({ root: { start, end, leading: start * 100, trailing: (count - end) * 100 } });
  const Row = ({ i }) => {
    useChannel('feed', 'ticker', { params: { i }, onData() {} }); return h('text', {}, i);
  };
  const render = () => h('list', { count, height: () => 100, row: i => h(Row, { key: String(i), i }) });
  prepare(render, view, window(0, 1)); prepareWindows(window(0, 2)); commit();
  const first = requests(); assert.equal(first.length, 2);
  for (const open of first) ack(open.id); await flush();
  prepare(render, view, window(0, 2)); commit();
  prepareWindows(window(1, 3)); commit();
  const [third] = requests(); assert.equal(third.kind, 'ch.open');
  ack(third.id); await flush();
  prepareWindows(window(0, 2)); commit(); assert.deepEqual(requests(), []);
  count = 1; prepare(render, view, window(0, 1)); commit();
  const closed = requests(); assert.equal(closed.length, 2);
  assert.ok(closed.every(frame => frame.kind === 'ch.close'));
});

test('callbacks change only at commit and a reset invalidates pending resource opens', async () => {
  const view = setup(), seen = [];
  const render = label => () => {
    useChannel('feed', 'ticker', { onData: value => seen.push([label, value]) }); return h('text');
  };
  describe(render('A'), view); const [open] = requests(); ack(open.id); await flush();
  prepare(render('B'), view); discard();
  receiveActionResponse({ v: 2, kind: 'ch.data', id: open.id, seq: 0, payload: 1 });
  describe(render('B'), view);
  receiveActionResponse({ v: 2, kind: 'ch.data', id: open.id, seq: 1, payload: 2 });
  assert.deepEqual(seen, [['A', 1], ['B', 2]]);
  assert.deepEqual(requests(), []);
  reset();
  const next = setup(); describe(render('C'), next); const [pending] = requests();
  reset(); await flush();
  assert.equal(receiveActionResponse({ v: 2, kind: 'ch.ack', id: pending.id, ok: true }), false);
  assert.deepEqual(requests(), []); assert.deepEqual(seen, [['A', 1], ['B', 2]]);
});
