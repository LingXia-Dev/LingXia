import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const source = readFileSync(new URL('../../../crates/lingxia-lxapp/src/appservice/scripts/Page.js', import.meta.url), 'utf8');
const context = vm.createContext({ setTimeout, clearTimeout, console, AbortController, AbortSignal });
vm.runInContext(`
globalThis.acks = [];
globalThis.PageSvc = class {
  constructor(config) { this.config = config; }
  _setData(ops, ack) { acks.push({ ops: JSON.parse(ops), ack }); return Promise.resolve(); }
};
`, context);
vm.runInContext(source, context);
// As the Logic worker does once its timer module is up.
vm.runInContext('__lxCaptureTimers()', context);
const run = (script) => vm.runInContext(script, context);
run(`__registerPage('home', { data: { count: 0, profile: null } });
globalThis.page = __LX_CREATE_PAGE__('home');
page.setData({ count: 1 });
globalThis.flushed = false;
globalThis.pending = page.flush().then(() => { flushed = true; });`);
assert.equal(run('page.data.count'), 1);
await new Promise(resolve => setTimeout(resolve, 30));
assert.equal(run('flushed'), false);
run('acks[0].ack()');
await run('pending');
assert.equal(run('flushed'), true);
run('page.setData({ count: 2 });');
await new Promise(resolve => setTimeout(resolve, 30));
run('globalThis.secondDone = false; globalThis.second = page.flush().then(() => { secondDone = true; });');
await new Promise(resolve => setTimeout(resolve, 30));
assert.equal(run('secondDone'), false, 'empty flush must wait for the in-flight batch');
run('acks[1].ack()');
await run('second');
assert.equal(run('secondDone'), true);
run(`page.setPath(['profile', 'name'], 'Alice')`);
assert.equal(run('page.data.profile.name'), 'Alice');
assert.throws(() => run('page.setData({ count: () => 1 })'), /JSON/);
assert.throws(() => run('page.setData({ count: new Date() })'), /class instances/);
assert.throws(() => run('const cyclic = {}; cyclic.self = cyclic; page.setData({ count: cyclic });'), /acyclic/);
// A write the View never receives must not resolve flush. Native reports the
// outcome, because it calls back on the discarded paths too.
run('page.setData({ count: 3 }); globalThis.dropped = page.flush(); dropped.catch(() => {});');
await new Promise(resolve => setTimeout(resolve, 10));
run('acks[acks.length - 1].ack("dropped")');
await assert.rejects(run('dropped'), /discarded/);
// "deferred" still arrives, through the bridge-ready snapshot.
run('page.setData({ count: 4 }); globalThis.deferred = page.flush();');
await new Promise(resolve => setTimeout(resolve, 10));
run('acks[acks.length - 1].ack("deferred")');
await run('deferred');
run('globalThis.unloaded = page.flush(); unloaded.catch(() => {}); page._cancelPendingSetData();');
await assert.rejects(run('unloaded'), /unloaded/);
await assert.rejects(run('page.flush()'), /unloaded/);
assert.throws(() => run(`__registerPage('bad', { flush() {} }); __LX_CREATE_PAGE__('bad');`), /under actions/);

// `this.signal` lives as long as the page: aborted when the runtime retires it.
run(`globalThis.signalPage = __LX_CREATE_PAGE__('home');
globalThis.heardAbort = false;
signalPage.signal.addEventListener('abort', () => { heardAbort = true; });`);
assert.equal(run('signalPage.signal.aborted'), false);
run('signalPage._cancelPendingSetData()');
assert.equal(run('signalPage.signal.aborted'), true);
assert.equal(run('heardAbort'), true);
assert.throws(
  () => run(`__registerPage('taken', { signal: null }); __LX_CREATE_PAGE__('taken');`),
  /not a lifecycle hook/,
);
// The opener's result settles once, after the page leaves.
const settled = async (expr) => JSON.parse(JSON.stringify(await run(expr)));
run(`globalThis.resultPage = __LX_CREATE_PAGE__('home');
globalThis.first = resultPage._bindResult();
globalThis.second = resultPage._bindResult();`);
assert.deepEqual(await settled('first'), { status: 'canceled', reason: 'opener' });
run(`resultPage.setResult({ id: 7 });
resultPage._settleResult('user');
resultPage._settleResult('app_closed');`);
assert.deepEqual(await settled('second'), { status: 'ok', value: { id: 7 } });
run(`globalThis.third = resultPage._bindResult(); resultPage._settleResult('user');`);
assert.deepEqual(await settled('third'), { status: 'canceled', reason: 'user' }, 'a new opener starts without a result');
assert.throws(() => run('resultPage.setResult(new Date())'), /class instances/);
run(`globalThis.left = 0; resultPage._leave = () => { left += 1; };
globalThis.fourth = resultPage._bindResult(); resultPage.close('done'); resultPage._settleResult('user');`);
assert.equal(run('left'), 1);
assert.deepEqual(await settled('fourth'), { status: 'ok', value: 'done' });
console.log('Page state validation, acknowledgement, teardown and result checks passed.');

// definePage: setup's hooks and actions run as the page; update diffs a draft.
run(`globalThis.define = (def) => Object.defineProperty(def, Symbol.for('lingxia.page'), { value: true });
__registerPage('order', define({
  data: { list: [{ n: 1 }, { n: 2 }], meta: { a: 1, b: 2 } },
  setup(page) {
    page.update(d => { d.meta.a = 10; });
    globalThis.orderPage = page;
    return {
      onLoad(params) { globalThis.loadedWith = params; },
      actions: { bump() { page.update(d => { d.list[1].n = 3; delete d.meta.b; }); } },
    };
  },
}));
globalThis.orderSvc = __LX_CREATE_PAGE__('pages/order/index', 'order', 'o1', false, JSON.stringify({ name: 'order', params: { id: 7 } }));
acks.length = 0;`);
assert.equal(run('orderPage.data.meta.a'), 10, 'an update in setup is the initial data');
assert.equal(run('orderPage.name'), 'order');
assert.equal(run('orderPage.params.id'), 7);
run("orderSvc.config.onLoad({ id: 8 })");
assert.equal(run('orderPage.params.id'), 8, 'onLoad refreshes params');
run("orderSvc.config.actions.bump()");
await new Promise(resolve => setTimeout(resolve, 30));
assert.deepEqual(JSON.parse(JSON.stringify(run('acks[0].ops'))), [
  { op: 'replace', path: '/list/1/n', value: 3 },
  { op: 'remove', path: '/meta/b' },
], 'update sends only what changed');
assert.throws(() => run('orderPage.update(d => { orderPage.update(e => {}); })'), /inside page.update/);
assert.throws(() => run('orderPage.update(async d => { d.list = []; })'), /synchronous/);
assert.throws(() => run('orderPage.update(d => ({ ...d }))'), /in place/);
assert.equal(run('orderPage.data.list.length'), 2, 'a rejected recipe changes nothing');
// The draft copies only what a recipe writes; these are the writes a naive copy-on-write loses.
run(`orderPage.update(d => { d.extra = { a: 1 }; d.extra.c = 2; });
orderPage.update(d => { d.list.push({ n: 4 }); d.list[0].n = 9; });
orderPage.update(d => { d.kept = d.list.filter(item => item.n > 3); });`);
assert.deepEqual(JSON.parse(JSON.stringify(run('orderPage.data.extra'))), { a: 1, c: 2 }, 'a replaced object keeps edits made through the draft');
assert.deepEqual(JSON.parse(JSON.stringify(run('orderPage.data.list'))), [{ n: 9 }, { n: 3 }, { n: 4 }], 'a grown array keeps edited items');
assert.deepEqual(JSON.parse(JSON.stringify(run('orderPage.data.kept'))), [{ n: 9 }, { n: 4 }], 'drafts read out of a derived array');
run('orderPage.update(d => { d.kept[0].n = 1; })');
assert.equal(run('orderPage.data.list[0].n'), 9, 'a derived array shares nothing with its source');
assert.throws(() => run('orderPage.update(d => { d.self = d; })'), /acyclic/);
assert.throws(() => run('orderPage.update(d => { d.when = new Date(); })'), /class instances/);
assert.equal(run('orderPage.data.when'), undefined, 'a rejected write changes nothing');
run('globalThis.leaked = null; orderPage.update(d => { leaked = d.extra; })');
assert.throws(() => run('leaked.a = 5'), /only usable inside the recipe/);
assert.throws(
  () => run(`__registerPage('loose', define({ setup: () => ({ save() {} }) })); __LX_CREATE_PAGE__('loose', null, 'l1');`),
  /under actions/,
);
assert.throws(
  () => run(`__registerPage('later', define({ setup: async () => ({}) })); __LX_CREATE_PAGE__('later', null, 'l2');`),
  /synchronous/,
);
run('orderSvc._cancelPendingSetData(); orderPage.update(d => { d.meta.a = 0; });');
assert.equal(run('orderPage.data.meta.a'), 10, 'update after unload is ignored');

// A View `<link to>`: back pops, a push settles when the page is on screen.
run(`globalThis.navs = [];
globalThis.lx = { nav: {
  back(n) { navs.push(['back', n]); return Promise.resolve(); },
  go(target) { navs.push(['go', target.page ?? target.tab]); return target.page ? { opened: Promise.resolve() } : Promise.resolve(); },
} };`);
await run(`__lxNavForView({ back: 2 })`);
await run(`__lxNavForView({ page: 'detail' })`);
await run(`__lxNavForView({ tab: 'home' })`);
assert.equal(run('JSON.stringify(navs)'), JSON.stringify([['back', 2], ['go', 'detail'], ['go', 'home']]));

// Actions are dispatched from their own map: no name is reserved, and `this` is undefined.
run(`globalThis.closed = 0;
__registerPage('dialog', define({
  data: { byId: { '7': { n: 1 } }, counts: {} },
  setup(page) {
    globalThis.dialogPage = page;
    return { actions: {
      close() { 'use strict'; closed += 1; globalThis.closeThis = this; },
      flush() { return 'flush'; }, signal() { return 'signal'; }, setData() { return 'setData'; },
      route() { return 'route'; }, page() { return 'page'; }, data() { return 'data'; }, onLoad() { return 'onLoad action'; },
    } };
  },
}));
globalThis.dialogSvc = __LX_CREATE_PAGE__('pages/dialog/index', 'dialog', 'd1', false, JSON.stringify({ name: 'dialog' }));
acks.length = 0;`);
const dispatch = (name) => run(`dialogSvc.config.actions[${JSON.stringify(name)}].call(undefined)`);
dispatch('close');
assert.equal(run('closed'), 1, 'an action named close runs');
assert.equal(run('closeThis'), undefined, 'an action sees this as undefined');
for (const name of ['flush', 'signal', 'setData', 'route', 'page', 'data']) {
  assert.equal(dispatch(name), name, `an action may be named ${name}`);
}
assert.equal(dispatch('onLoad'), 'onLoad action', 'an action named like a hook stays an action');
assert.equal(run('typeof dialogSvc.config.onLoad'), 'function', 'the hook is the runtime one');
assert.equal(run('typeof dialogSvc.flush'), 'function', 'the runtime member is untouched');

// A numeric-looking key in an object is a plain key.
run(`dialogPage.update(d => { d.byId['12'] = { n: 5 }; d.byId['7'].n = 2; d.counts['2024'] = 5; })`);
assert.deepEqual(JSON.parse(JSON.stringify(run('dialogPage.data'))), {
  byId: { '7': { n: 2 }, '12': { n: 5 } }, counts: { '2024': 5 },
});
await new Promise(resolve => setTimeout(resolve, 30));
assert.deepEqual(JSON.parse(JSON.stringify(run('acks[0].ops'))), [
  { op: 'replace', path: '/byId/7/n', value: 2 },
  { op: 'add', path: '/byId/12', value: { n: 5 } },
  { op: 'add', path: '/counts/2024', value: 5 },
]);
run(`dialogPage.update(d => { delete d.byId['12']; })`);
assert.equal(run(`'12' in dialogPage.data.byId`), false, 'a numeric-looking key is removed, not spliced');
console.log('definePage action map and numeric object keys passed.');

// page.env.watch: the current value first, then flips; `{ signal }` ends it.
run(`globalThis.envNow = { sizeClass: 'compact', canDock: false };
globalThis.envSubs = new Set();
globalThis.__LX_PAGE_NATIVE__ = {
  env: () => envNow,
  envWatch(cb) { envSubs.add(cb); return () => envSubs.delete(cb); },
};
globalThis.envSeen = [];
globalThis.envStop = new AbortController();
dialogPage.env.watch(e => envSeen.push(e.sizeClass), { signal: envStop.signal });`);
assert.deepEqual(JSON.parse(run('JSON.stringify(envSeen)')), ['compact'], 'watch delivers the current value at once');
run(`envNow = { sizeClass: 'regular', canDock: true }; for (const cb of envSubs) cb(envNow);`);
assert.deepEqual(JSON.parse(run('JSON.stringify(envSeen)')), ['compact', 'regular']);
run('envStop.abort()');
assert.equal(run('envSubs.size'), 0, 'aborting the signal unsubscribes');
run(`globalThis.envLate = []; dialogPage.env.watch(e => envLate.push(e), { signal: AbortSignal.abort() });`);
assert.equal(run('envLate.length'), 0, 'an aborted signal subscribes nothing');
assert.throws(() => run('dialogPage.env.watch(() => {}, { signal: 1 })'), /\{ signal \}/);
console.log('page.env.watch delivery and signal passed.');
