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
console.log('Page state validation, acknowledgement, and teardown checks passed.');
