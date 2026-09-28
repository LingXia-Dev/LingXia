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
let heard = 0;
const stop = runtime.subscribePageSnapshot(() => heard++);
pushState({ title: 'Hello' }, { rev: 1, initial: true });
await waiting;
assert.equal(runtime.isPageReady(), true);
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
const { __lx_define_page_bridge } = await import('../../../tools/lingxia-cli/templates/builder-frameworks/page_bridge_runtime.js');
for (const save of [actions.save, __lx_define_page_bridge('save', 'call')]) {
const action = save({ title: 'Draft' });
let settled = false;
void action.then(() => { settled = true; });
await Promise.resolve();
assert.equal(settled, false);
finishAction('saved');
assert.equal(await action, 'saved');
}
console.log('page runtime: ok');
