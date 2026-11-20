import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const source = readFileSync(new URL('../../../crates/lingxia-lxapp/src/appservice/scripts/Page.js', import.meta.url), 'utf8');
const context = vm.createContext({ setTimeout, clearTimeout, console, AbortController, AbortSignal });
vm.runInContext(`
globalThis.acks = [];
globalThis.PageSvc = class {
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
assert.throws(() => run(`__registerPage('bad', { flush() {} }); __LX_CREATE_PAGE__('bad');`), /reserved/);

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
  /reserved by the runtime/,
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
run('orderSvc.onLoad({ id: 8 })');
assert.equal(run('orderPage.params.id'), 8, 'onLoad refreshes params');
run('orderSvc.bump()');
await new Promise(resolve => setTimeout(resolve, 30));
assert.deepEqual(JSON.parse(JSON.stringify(run('acks[0].ops'))), [
  { op: 'replace', path: '/list/1/n', value: 3 },
  { op: 'remove', path: '/meta/b' },
], 'update sends only what changed');
assert.throws(() => run('orderPage.update(d => { orderPage.update(e => {}); })'), /inside page.update/);
assert.throws(() => run('orderPage.update(async d => { d.list = []; })'), /synchronous/);
assert.throws(() => run('orderPage.update(d => ({ ...d }))'), /in place/);
assert.equal(run('orderPage.data.list.length'), 2, 'a rejected recipe changes nothing');
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
