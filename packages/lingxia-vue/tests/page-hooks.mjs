import assert from 'node:assert/strict';

// The bridge's state push and the host's pushes, driven by hand.
let pushState = null;
globalThis.window = {
  __LX_BRIDGE_CFG: {
    os: 'Android',
    hostClass: 'mobile',
    displayLanguage: 'en-US',
    surfaceContext: { sizeClass: 'compact', width: 390, height: 844, aside: false },
    surfaceContextRevision: 1,
  },
  LingXiaBridge: {
    state: { subscribe: (callback) => { pushState = callback; return () => {}; } },
    raw: { call: () => new Promise(() => {}) },
  },
  __pageBridge: { __names: ['save'], __modes: { save: 'notify' } },
};
const { nextTick } = await import('vue');
// Enough of a document for the runtime's own <html> stamps and CSS variables
// (after Vue, whose DOM runtime would take it for a real one).
globalThis.document = {
  documentElement: { style: { setProperty() {} }, setAttribute() {} },
};
const { useLxHost, useLxPage } = await import('../dist/test/hooks.mjs');

// `data` is updated in place, so a destructured `data` stays live.
const { data, actions } = useLxPage();
pushState({ title: 'First', items: [1] }, { rev: 1, initial: true });
assert.equal(data.title, 'First');
assert.equal(typeof actions.save, 'function');
assert.equal(useLxPage().actions, actions, 'one actions object for the page');
pushState({ title: 'Second', items: [1, 2] }, { rev: 2, initial: false });
await nextTick();
assert.equal(data.title, 'Second', 'the destructured data follows setData');
assert.deepEqual([...data.items], [1, 2]);
pushState({ items: [] }, { rev: 3, initial: false });
assert.equal('title' in data, false, 'a key Logic dropped is dropped');

// The host facts are one readonly reactive object that follows a change.
const host = useLxHost();
assert.equal(host.sizeClass, 'compact');
assert.equal(host.formFactor, 'mobile');
window.__lingxiaApplySurfaceContext({ sizeClass: 'regular', width: 900, height: 700, aside: true }, 2);
assert.equal(host.sizeClass, 'regular');
assert.equal(host.aside, true);
window.__lingxiaApplyDisplayLanguage('zh-CN');
assert.equal(host.displayLanguage, 'zh-CN');
const warn = console.warn;
console.warn = () => {}; // Vue warns about the rejected write; that is the point.
host.sizeClass = 'compact';
console.warn = warn;
assert.equal(host.sizeClass, 'regular', 'readonly: a page cannot write host facts');

console.log('vue page hooks: ok');
