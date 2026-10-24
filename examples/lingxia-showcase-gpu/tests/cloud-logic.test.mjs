import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const require = createRequire(new URL('../../../packages/lingxia-ui/package.json', import.meta.url));
const { buildSync } = require('esbuild');
const source = buildSync({
  entryPoints: [fileURLToPath(new URL('../lxapp/pages/cloud/index.ts', import.meta.url))],
  bundle: true, write: false, platform: 'neutral', format: 'iife', logLevel: 'silent',
}).outputFiles[0].text;
const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
function fixture() {
  let page, disposed = false, lateWrites = 0, listeners = 0;
  const pending = [];
  const lx = {
    navigationBar: { async update() {} },
    auth: { async list() { return []; } },
    cloud: {
      async invoke(name, payload) {
        if (name === 'fail') throw Error('Intentional function failure');
        return { name, payload };
      },
      mqtt: {
        getStatus() { return { state: 'connected' }; },
        onStatusChange() { listeners++; return () => listeners--; },
        subscribe() { const next = deferred(); pending.push(next); return next.promise; },
      },
    },
  };
  vm.runInNewContext(source, {
    lx, console: { log() {}, warn() {}, error() {} },
    getCurrentPages: () => disposed ? [] : [page],
    Page(definition) {
      page = { ...definition, data: structuredClone(definition.data), setData(patch) {
        if (disposed) lateWrites++;
        Object.assign(this.data, patch);
      } };
    },
  });
  return {
    page, pending, lx, lateWrites: () => lateWrites, listeners: () => listeners,
    async unload() { disposed = true; await page.onUnload(); },
  };
}
function subscription() {
  const next = deferred();
  return {
    closes: 0,
    async close() { this.closes++; next.resolve({ done: true }); },
    [Symbol.asyncIterator]() { return this; },
    next() { return next.promise; },
  };
}

test('subscription completed after unload closes without writing into the disposed page', async () => {
  const f = fixture(), late = subscription();
  const start = f.page.startMqttDemo();
  assert.equal(f.listeners(), 1);
  await f.unload();
  f.pending[0].resolve(late); await start;
  assert.equal(late.closes, 1);
  assert.equal(f.page.mqttSubscription, null);
  assert.equal(f.listeners(), 0);
  assert.equal(f.lateWrites(), 0);
});

test('stop cancels a pending subscription and a newer start owns the returned handle', async () => {
  const f = fixture(), old = subscription(), current = subscription();
  const first = f.page.startMqttDemo();
  await f.page.stopMqttDemo();
  const second = f.page.startMqttDemo();
  assert.equal(f.pending.length, 2);
  f.pending[1].resolve(current); await second;
  f.pending[0].resolve(old); await first;
  assert.equal(old.closes, 1);
  assert.equal(f.page.mqttSubscription, current);
  assert.equal(f.page.data.mqttSubscribed, true);
  await f.unload(); assert.equal(current.closes, 1);
});

test('late rejection after unload does not replace page status', async () => {
  const f = fixture(), start = f.page.startMqttDemo();
  await f.unload();
  f.pending[0].reject(Error('connection lost')); await start;
  assert.equal(f.lateWrites(), 0);
});

test('a slow close cannot overwrite a newer subscription state', async () => {
  const f = fixture(), old = subscription(), current = subscription(), closing = deferred();
  old.close = () => closing.promise;
  const first = f.page.startMqttDemo();
  f.pending[0].resolve(old); await first;
  const stop = f.page.stopMqttDemo();
  const second = f.page.startMqttDemo();
  f.pending[1].resolve(current); await second;
  closing.resolve(); await stop;
  assert.equal(f.page.mqttSubscription, current);
  assert.equal(f.page.data.mqttSubscribed, true);
  assert.equal(f.page.data.mqttStatus, 'Subscribed to demo/mqtt');
  await f.unload();
});

test('function results and failures retain the original Logic contract', async () => {
  const f = fixture();
  await f.page.callNamedFunction({ name: 'echo' });
  assert.equal(f.page.data.functionsStatus, 'echo succeeded');
  assert.equal(JSON.parse(f.page.data.functionsLastResult).payload.hello, 'world');
  await f.page.callNamedFunction({ name: 'fail' });
  assert.equal(f.page.data.functionsStatus, 'Intentional function failure');
  assert.equal(f.page.data.functionsLastResult, '');
  assert.equal(f.page.data.functionsLastCall, 'fail');
});

test('an older cloud function response cannot replace the latest call', async () => {
  const f = fixture(), first = deferred(), second = deferred();
  f.lx.cloud.invoke = name => name === 'echo' ? first.promise : second.promise;
  f.page.data.functionsLastResult = 'previous result';
  const oldCall = f.page.callNamedFunction({ name: 'echo' });
  assert.equal(f.page.data.functionsLastResult, '');
  const currentCall = f.page.callNamedFunction({ name: 'whoami' });
  second.resolve({ tenantId: 'tenant-b' }); await currentCall;
  first.resolve({ hello: 'stale' }); await oldCall;
  assert.equal(f.page.data.functionsLastCall, 'whoami');
  assert.equal(f.page.data.functionsStatus, 'whoami succeeded');
  assert.equal(JSON.parse(f.page.data.functionsLastResult).tenantId, 'tenant-b');
});

test('function success and failure after unload do not write to the removed page', async () => {
  for (const rejected of [false, true]) {
    const f = fixture(), result = deferred();
    f.lx.cloud.invoke = () => result.promise;
    const call = f.page.callNamedFunction({ name: 'echo' });
    await f.unload();
    if (rejected) result.reject(Error('late failure'));
    else result.resolve({ hello: 'late result' });
    await call;
    assert.equal(f.lateWrites(), 0);
  }
});
