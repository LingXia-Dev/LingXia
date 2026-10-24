import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
const require = createRequire(new URL('../../../packages/lingxia-ui/package.json', import.meta.url));
const source = require('esbuild').buildSync({ entryPoints: [fileURLToPath(new URL('../lxapp/pages/todo/index.ts', import.meta.url))], bundle: true, write: false, format: 'iife' }).outputFiles[0].text;
function fixture(seed = {}) {
  const saved = new Map(Object.entries(seed));
  let page, readFailure = false, writeFailure = false, writes = 0, reads = 0, hold, readHold;
  const storage = {
    async get(key) { reads++; if (readHold) await readHold; if (readFailure) throw Error('read unavailable'); return structuredClone(saved.get(key)); },
    async set(key, value) { if (hold) await hold; if (writeFailure) throw Error('disk full'); writes++; saved.set(key, structuredClone(value)); },
  };
  vm.runInNewContext(source, { lx: { getStorage: () => storage }, Page(definition) {
    page = { ...definition, data: structuredClone(definition.data), setData(patch) { Object.assign(this.data, patch); }, async flush() {} };
  } });
  return { page, saved, writes: () => writes, reads: () => reads, readHold(value) { readHold = value; }, readFailure(value) { readFailure = value; }, writeFailure(value) { writeFailure = value; }, hold(value) { hold = value; } };
}
test('empty legacy task lists restore their saved filter and migrate in one write', async () => {
  const f = fixture({ 'todo:todos': [], 'todo:filter': 'completed' });
  await f.page.onLoad(); assert.equal(f.page.data.currentFilter, 'completed');
  await f.page.setFilter({ filter: 'active' });
  assert.equal(f.writes(), 1); assert.equal(f.saved.get('todo:state').currentFilter, 'active');
  const next = fixture(Object.fromEntries(f.saved)); await next.page.onLoad();
  assert.equal(next.page.data.todos.length, 0); assert.equal(next.page.data.currentFilter, 'active');
});
test('failed writes retain visible state and retries create exactly one task', async () => {
  const f = fixture(); await f.page.onLoad(); f.writeFailure(true);
  await assert.rejects(f.page.addTodo({ text: 'Keep my draft' }), /disk full/);
  assert.equal(f.page.data.todos.length, 0); assert.equal(f.writes(), 0);
  f.writeFailure(false); await f.page.addTodo({ text: 'Keep my draft' });
  assert.equal(f.page.data.todos.length, 1); assert.equal(f.saved.get('todo:state').todos.length, 1);
  f.writeFailure(true); await assert.rejects(f.page.toggleAll(), /disk full/);
  assert.equal(f.page.data.todos[0].completed, false);
});
test('read failure prevents overwriting saved tasks until retry succeeds', async () => {
  const f = fixture({ 'todo:todos': [{ id: 'old', text: 'Saved', completed: false }] });
  f.readFailure(true); await f.page.onLoad(); assert.match(f.page.data.loadError, /Retry/);
  await assert.rejects(f.page.addTodo({ text: 'New' }), /not ready/); assert.equal(f.writes(), 0);
  f.readFailure(false); await f.page.reload(); assert.equal(f.page.data.loadError, ''); assert.equal(f.page.data.todos[0].id, 'old');
});
test('pending save excludes concurrent edits and does not publish after unload', async () => {
  const f = fixture(); await f.page.onLoad(); let finish;
  f.hold(new Promise(resolve => { finish = resolve; }));
  const pending = f.page.addTodo({ text: 'Pending' });
  await assert.rejects(f.page.setFilter({ filter: 'active' }), /not ready/);
  f.page.onUnload(); finish(); await pending;
  assert.equal(f.page.data.todos.length, 0); assert.equal(f.saved.get('todo:state').todos.length, 1);
});

test('repeated retry shares the in-flight load and cannot issue a stale second read', async () => {
  const f = fixture(); let finish;
  f.readHold(new Promise(resolve => { finish = resolve; }));
  const first = f.page.onLoad();
  await f.page.reload(); assert.equal(f.reads(), 1);
  finish(); await first; assert.equal(f.page.data.loading, false);
  assert.equal(f.reads(), 4);
});
