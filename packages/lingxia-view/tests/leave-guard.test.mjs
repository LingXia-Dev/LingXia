import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createLeaveGuards } from '../src/runtime/leave-guard.js';
import {
  h, prepare, commit, discard, describe, reset, createView, useLeaveGuard,
  requestLeave, configureActions, takeActionRequests, receiveActionResponse,
} from '../src/internal.js';

const flush = async () => { for (let i = 0; i < 12; i++) await Promise.resolve(); };
function setup() {
  const calls = [], errors = [], failures = [];
  const guards = createLeaveGuards(async (name, params) => { calls.push({ name, params }); }, error => errors.push(error), message => failures.push(message));
  const resource = () => guards.resource({ value: { pending: null, requests: 0, error: null }, set() {} });
  return { guards, calls, errors, failures, resource };
}

test('keep editing cancels; repeated back produces one question; discard preserves reason', async () => {
  const { guards, calls, resource } = setup(), guard = resource();
  guard.commit({ enabled: true }); await flush();
  assert.deepEqual(calls.shift(), { name: 'navigation.setLeaveGuard', params: { enabled: true } });
  guards.request('back'); guards.request('back');
  assert.equal(guard.state.requests, 1);
  assert.equal(guard.state.pending, 'back');
  guard.answer(false); await flush();
  assert.equal(guard.state.pending, null); assert.deepEqual(calls, []);
  guards.request('home'); guard.answer(true); await flush();
  assert.deepEqual(calls, [{ name: 'navigation.leave', params: { reason: 'home' } }]);
});

test('removal and bridge reset invalidate pending answers and re-register retained guards', async () => {
  const { guards, calls, resource } = setup(), guard = resource();
  guard.commit({ enabled: true }); await flush(); calls.length = 0;
  guards.request('back'); guards.reset();
  assert.throws(() => guard.request('home'), /stale guard/);
  guard.answer(true); await flush();
  assert.deepEqual(calls, []); assert.equal(guard.state.pending, null);
  guard.commit({ enabled: true }); await flush();
  assert.deepEqual(calls.pop(), { name: 'navigation.setLeaveGuard', params: { enabled: true } });
  guards.request('back'); guard.dispose(); guard.answer(true); await flush();
  assert.deepEqual(calls, [{ name: 'navigation.setLeaveGuard', params: { enabled: false } }]);
  assert.throws(() => guard.request('back'), /stale guard/);
});

test('nested guards answer newest first and all must accept', async () => {
  const { guards, calls, resource } = setup(), first = resource(), last = resource();
  first.commit({ enabled: true }); last.commit({ enabled: true }); await flush(); calls.length = 0;
  guards.request('back');
  assert.equal(first.state.pending, null); assert.equal(last.state.pending, 'back');
  last.answer(true); await flush();
  assert.equal(first.state.pending, 'back');
  first.answer(false); await flush(); assert.deepEqual(calls, []);
});

test('disabled guard clears an open confirmation without navigating', async () => {
  const { guards, calls, resource } = setup(), guard = resource();
  guard.commit({ enabled: true }); await flush(); calls.length = 0;
  guards.request('back'); guard.commit({ enabled: false }); await flush();
  assert.equal(guard.state.pending, null);
  assert.deepEqual(calls, [{ name: 'navigation.setLeaveGuard', params: { enabled: false } }]);
  assert.throws(() => guards.request('invalid'), /invalid reason/);
  assert.throws(() => guard.answer('yes'), /boolean answer/);
});

test('render candidates register only on commit and navigation requires an explicit answer', async () => {
  reset(); configureActions({ __names: [], __modes: {} });
  const view = createView({}, {});
  let guard;
  const render = () => { guard = useLeaveGuard('draft', { enabled: true }); return h('text', {}, guard.pending ?? 'saved'); };
  prepare(render, view); discard(); await flush();
  assert.deepEqual(takeActionRequests(), []);
  prepare(render, view); commit(); await flush();
  const [registration] = takeActionRequests().map(JSON.parse);
  assert.equal(registration.method, 'host.navigation.setLeaveGuard');
  assert.equal(registration.cap, 'host');
  receiveActionResponse({ v: 2, kind: 'res', id: registration.id, ok: true, result: null });
  await flush();
  requestLeave('back');
  describe(render, view);
  assert.equal(guard.pending, 'back');
  assert.deepEqual(takeActionRequests(), []);
  guard.answer(true); await flush();
  const [leave] = takeActionRequests().map(JSON.parse);
  assert.equal(leave.method, 'host.navigation.leave');
  assert.deepEqual(leave.params, { reason: 'back' });
  reset();
});

test('leave controls cannot run during rendering', () => {
  reset(); configureActions({ __names: [], __modes: {} });
  const view = createView({}, {});
  assert.throws(() => prepare(() => {
    useLeaveGuard('draft', { enabled: true }).answer(true);
    return h('text');
  }, view), /mutation during render/);
  discard(); reset();
});

test('registration serializes acknowledgements and converges to the committed enabled state', async () => {
  const calls = [], replies = [];
  const guards = createLeaveGuards((name, params) => {
    calls.push({ name, params });
    return new Promise(resolve => replies.push(resolve));
  }, error => { throw error; });
  const guard = guards.resource({ value: { pending: null, requests: 0, error: null }, set() {} });
  guard.commit({ enabled: true }); await flush();
  guard.commit({ enabled: false }); await flush();
  assert.equal(calls.length, 1);
  replies.shift()(); await flush();
  assert.deepEqual(calls.map(call => call.params.enabled), [true, false]);
  replies.shift()(); await flush();
  guard.dispose();
});

test('registration failure is visible to the page and reported', async () => {
  const errors = [];
  const guards = createLeaveGuards(async () => { throw Error('registration unavailable'); }, error => errors.push(error));
  const guard = guards.resource({ value: { pending: null, requests: 0, error: null }, set() {} });
  guard.commit({ enabled: true }); await flush();
  assert.deepEqual(guard.state.error, { code: 'UI_ERROR', message: 'registration unavailable' });
  assert.equal(errors.length, 1);
});

test('hooks, JSX and commits outside their phase fail by code', () => {
  reset();
  assert.throws(() => useLeaveGuard('draft', { enabled: true }), /^Error: UI_SCOPE:/);
  assert.throws(() => h('text', {}, 'x'), /^Error: UI_SCOPE: JSX outside render/);
  assert.throws(() => commit(), /^Error: UI_COMMIT: no candidate/);
  assert.throws(() => describe(() => { useLeaveGuard('draft', {}); return h('text', {}, 'x'); }, {}), /UI_LEAVE: expected \{ enabled: boolean \}/);
});

test('sync tells the host a back handler is mounted, and clears it with the last one', async () => {
  const { guards, calls, resource } = setup();
  const guard = resource(), first = guards.backHandler(), second = guards.backHandler();
  guard.commit({ enabled: true }); await flush();
  assert.deepEqual(calls.splice(0), [{ name: 'navigation.setLeaveGuard', params: { enabled: true } }]);
  first.commit({ enabled: true, onBack() {} }); second.commit({ enabled: true, onBack() {} }); await flush();
  assert.deepEqual(calls.splice(0), [{ name: 'navigation.setLeaveGuard', params: { enabled: true, back: true } }]);
  first.dispose(); await flush();
  assert.deepEqual(calls.splice(0), [], 'one back handler is still mounted');
  second.commit({ enabled: false, onBack() {} }); await flush();
  assert.deepEqual(calls.splice(0), [{ name: 'navigation.setLeaveGuard', params: { enabled: true } }]);
});

test('a throwing or rejecting onBack fails as a handler does, and Back stays intercepted', async () => {
  const { guards, calls, errors, failures } = setup();
  const back = guards.backHandler();
  back.commit({ enabled: true, onBack() { throw Error('no seats'); } }); await flush();
  calls.length = 0;
  guards.request('back'); await flush();
  back.commit({ enabled: true, onBack: async () => { await 0; throw Error('later'); } });
  guards.request('back'); await flush();
  assert.deepEqual(failures, ['UI_HANDLER: onBack threw: no seats', 'UI_HANDLER: onBack rejected: later']);
  assert.deepEqual(errors, []);
  assert.deepEqual(calls, [], 'the page does not leave');
});
