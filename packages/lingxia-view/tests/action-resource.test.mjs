import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, describe, reset, configureActions, createView, useAction, takeActionRequests, takeActionErrors, receiveActionResponse } from '../src/internal.js';
const frames = () => takeActionRequests().map(JSON.parse);
const flush = async () => { for (let i = 0; i < 4; i++) await Promise.resolve(); };
const setup = () => { reset(); configureActions({ __names: ['save'], __modes: { save: 'call' } }); return createView({}, {}); };
const reply = (id, ok, value) => receiveActionResponse(ok
  ? { v: 2, kind: 'res', id, ok: true, result: value }
  : { v: 2, kind: 'res', id, ok: false, error: { code: 'E_DISK', message: value } });

test('useAction tracks the latest call: pending, then the result or the error, never a rejection', async () => {
  const view = setup(); let save;
  const render = () => { save = useAction(view.actions.save); return h('text'); };
  describe(render, view);
  assert.deepEqual([save.pending, save.error], [false, null]);
  const first = save.run({ n: 1 });
  const [frame] = frames(); assert.equal(frame.method, 'save'); assert.deepEqual(frame.params, { n: 1 });
  assert.equal(save.pending, true);
  reply(frame.id, true, 'saved');
  assert.equal(await first, 'saved');
  assert.deepEqual([save.pending, save.error], [false, null]);
  const failed = save.run();
  const [next] = frames();
  reply(next.id, false, 'disk full');
  assert.equal(await failed, undefined, 'a failed run resolves');
  assert.deepEqual([save.pending, save.error], [false, { code: 'E_DISK', message: 'disk full' }]);
  await flush();
  assert.deepEqual(takeActionErrors(), [], 'the error is shown by the page, not reported');
  const retry = save.run();
  assert.equal(save.error, null, 'a new run clears the error');
  reply(frames()[0].id, true, 'ok'); await retry;
});

test('useAction keys by call order or by name, and refuses what is not an action', () => {
  const view = setup(); let a, b;
  describe(() => { a = useAction(view.actions.save); b = useAction('again', view.actions.save); return h('text'); }, view);
  assert.notEqual(a, b);
  assert.throws(() => describe(() => { useAction(async () => 1); return h('text'); }, view), /UI_ACTION/);
  assert.throws(() => describe(() => { useAction(); return h('text'); }, view), /UI_ACTION/);
});
