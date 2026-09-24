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

// Actions: never cached empty before the page's metadata exists, then one object.
assert.deepEqual(runtime.getPageActions(), {});
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

console.log('page runtime: ok');
