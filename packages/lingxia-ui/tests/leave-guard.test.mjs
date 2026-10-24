import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createLeaveGuards } from '../src/leave-guard.js';
import {
  h, prepare, commit, discard, describe, reset, createView, useLeaveGuard,
  requestLeave, configureActions, takeActionRequests, receiveActionResponse,
} from '../src/runtime.js';

const flush = async () => { for (let i = 0; i < 12; i++) await Promise.resolve(); };
function setup() {
  const calls = [], errors = [];
  const guards = createLeaveGuards(async (name, params) => { calls.push({ name, params }); }, error => errors.push(error));
  const resource = () => guards.resource({ value: { pending: null, requests: 0, error: null }, set() {} });
  return { guards, calls, errors, resource };
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
  const render = () => { guard = useLeaveGuard('draft', true); return h('text', {}, guard.pending ?? 'saved'); };
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
    useLeaveGuard('draft', true).answer(true);
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
  assert.equal(guard.state.error, 'registration unavailable');
  assert.equal(errors.length, 1);
});
