import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';

const source = readFileSync(new URL('../src/android/servo_bridge.js', import.meta.url), 'utf8');

test('deliver ordered bridge messages through asynchronous and failed image fetches', () => {
  const requests = [];
  class Image {
    set src(value) { requests.push({ image: this, url: new URL(value) }); }
  }
  const context = vm.createContext({ Image, URLSearchParams });
  vm.runInContext(source, context);
  const proxy = context.LingXiaProxy;
  proxy.postMessage('input:9');
  proxy.postMessage('submit');
  proxy.resolveEval(7, 'bound-token', '你好 & emoji 😀');
  assert.equal(requests.length, 1, 'later requests cannot overtake the first');
  assert.equal(requests[0].url.searchParams.get('message'), 'input:9');
  // The native endpoint returns 204, so Image normally completes via onerror.
  requests[0].image.onerror();
  assert.equal(requests.length, 2);
  assert.equal(requests[1].url.searchParams.get('message'), 'submit');
  requests[0].image.onload();
  assert.equal(requests.length, 2, 'a duplicate completion cannot release another request');
  requests[1].image.onload();
  assert.equal(requests.length, 3);
  assert.equal(requests[2].url.pathname, '/eval');
  assert.equal(requests[2].url.searchParams.get('token'), 'bound-token');
  assert.equal(requests[2].url.searchParams.get('result'), '你好 & emoji 😀');
  requests[2].image.onerror();
  proxy.postMessage('next gesture');
  assert.equal(requests.length, 4, 'an empty queue resumes when new input arrives');
  assert.deepEqual(requests.map(({ url }) => url.searchParams.get('sequence')), ['0', '1', '2', '3']);
});
