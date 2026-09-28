import assert from 'node:assert/strict';

// A bridge whose snapshot request always fails: the host never answers, as for
// a page with no Logic. The state push is driven by hand.
let pushState = null;
let snapshotRequests = 0;
globalThis.window = {
  LingXiaBridge: {
    state: { subscribe: (callback) => { pushState = callback; return () => {}; } },
    raw: { call: () => { snapshotRequests += 1; return Promise.reject(new Error('no page service')); } },
  },
};
const runtime = await import('../dist/test/runtime.mjs');
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// An early read must not hand out a permanently empty, apparently typed object.
assert.throws(() => runtime.getPageActions(), /Page actions are not ready/);
window.__pageBridge = { __names: ['save'], __modes: { save: 'call' } };
const actions = runtime.getPageActions();
assert.equal(typeof actions.save, 'function');
assert.equal(runtime.getPageActions(), actions, 'one actions object for the page');

// Not ready yet; a deadline rejects, and leaves nothing behind.
assert.equal(runtime.isPageReady(), false);
await assert.rejects(runtime.whenPageReady({ timeoutMs: 20 }), /did not deliver the page state/);

// The fallback request retries a few times, backing off, then stops.
await sleep(2_300);
assert.equal(snapshotRequests, 4, 'one request and three retries, then no more');
await sleep(600);
assert.equal(snapshotRequests, 4, 'never asked again');

// `null` waits without limit and resolves on the host's push.
const waiting = runtime.whenPageReady({ timeoutMs: null });
// An action called before the first state is held, then sent.
const held = [];
const snapshotCall = window.LingXiaBridge.raw.call;
window.LingXiaBridge.raw.call = (name, payload) => { held.push(name); return Promise.resolve(payload); };
const early = actions.save({ id: 1 });
await sleep(10);
assert.deepEqual(held, [], 'not sent before the page is ready');
let heard = 0;
const stop = runtime.subscribePageSnapshot(() => heard++);
pushState({ title: 'Hello' }, { rev: 1, initial: true });
await waiting;
assert.deepEqual(await early, { id: 1 });
assert.deepEqual(held, ['save'], 'sent once the page is ready');
window.LingXiaBridge.raw.call = snapshotCall;
assert.equal(runtime.isPageReady(), true);

// After the page left, the host answers not-ready: the call is dropped, never reported.
let reported = 0;
const warn = console.warn;
console.warn = () => { reported += 1; };
window.LingXiaBridge.raw.call = () => Promise.reject(Object.assign(new Error('Bridge not ready'), { code: 'BRIDGE_NOT_READY' }));
let lateSettled = false;
actions.save({ late: true }).then(() => { lateSettled = true; }, () => { lateSettled = true; });
await sleep(10);
assert.equal(lateSettled, false, 'a departed page never settles the call');
assert.equal(reported, 0, 'and reports nothing');
console.warn = warn;
window.LingXiaBridge.raw.call = snapshotCall;
assert.deepEqual(runtime.getPageSnapshot(), { title: 'Hello' });
assert.equal(heard, 1);
await runtime.whenPageReady({ timeoutMs: 1 });
stop();

// Outside a dev session the data is left as pushed; in one, a View write
// throws at the write, at any depth.
assert.equal(Object.isFrozen(runtime.getPageSnapshot()), false);
window.__LX_BRIDGE_CFG = { dev: true };
pushState({ title: 'Dev', items: [{ id: 'a' }] }, { rev: 2, initial: false });
const dev = runtime.getPageSnapshot();
assert.throws(() => { dev.title = 'draft'; }, TypeError);
assert.throws(() => { dev.items.push({ id: 'b' }); }, TypeError);
assert.throws(() => { dev.items[0].id = 'draft'; }, TypeError);
assert.deepEqual(dev, { title: 'Dev', items: [{ id: 'a' }] });
delete window.__LX_BRIDGE_CFG;

// Long unary actions settle with the business operation, without a bridge timer.
let finishAction;
window.LingXiaBridge.raw.call = (name, payload, options) => {
  assert.equal(name, 'save');
  assert.deepEqual(payload, { title: 'Draft' });
  assert.equal(options.timeoutMs, 0);
  return new Promise(resolve => { finishAction = resolve; });
};
const action = actions.save({ title: 'Draft' });
let settled = false;
void action.then(() => { settled = true; });
await Promise.resolve();
assert.equal(settled, false);
finishAction('saved');
assert.equal(await action, 'saved');

// The automation hook calls the same unary wire path, with the JSON payload
// untouched, and refuses what the View could not call as a unary action.
const invoke = window.__lxInvokePageAction;
assert.equal(typeof invoke, 'function');
const bridgeMetadata = window.__pageBridge;
delete window.__pageBridge;
await assert.rejects(invoke('save'), { code: 'PAGE_ACTIONS_NOT_READY' });
window.__pageBridge = { __names: ['save', 'feed'], __modes: { save: 'call', feed: 'stream' } };
await assert.rejects(invoke('missing'), { code: 'BRIDGE_METHOD_NOT_FOUND', message: /actions: save, feed/ });
await assert.rejects(invoke('feed'), { code: 'PAGE_ACTION_NOT_UNARY' });
window.LingXiaBridge.raw.call = (name, payload, options) => {
  assert.equal(options.timeoutMs, 0);
  return Promise.resolve({ name, payload });
};
const eventShaped = { type: 'submit', detail: 1, extra: true };
assert.deepEqual(await invoke('save', eventShaped), { name: 'save', payload: eventShaped });
window.LingXiaBridge.raw.call = () => Promise.reject({ code: 'BRIDGE_INTERNAL_ERROR', message: 'boom' });
await assert.rejects(invoke('save'), { code: 'BRIDGE_INTERNAL_ERROR', message: 'boom' });
window.__pageBridge = bridgeMetadata;
console.log('page runtime: ok');
