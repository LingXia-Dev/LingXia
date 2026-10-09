import { test } from 'node:test';
import assert from 'node:assert/strict';
import { request } from 'node:http';
import { createAndroidDeviceFixture } from './android-device-fixture.mjs';

test('device input requires the capability URL and rejects shell-shaped arguments', async (t) => {
  const calls = [];
  const { server, token } = createAndroidDeviceFixture(async (args) => { calls.push(args); return ''; });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise((resolve) => { server.close(resolve); server.closeAllConnections(); }));
  const origin = `http://127.0.0.1:${server.address().port}`;
  const malformed = await new Promise((resolve, reject) => {
    const call = request({ hostname: '127.0.0.1', port: server.address().port, path: '//[' }, (response) => {
      response.resume();
      response.on('end', () => resolve(response.statusCode));
    });
    call.on('error', reject);
    call.end();
  });
  assert.equal(malformed, 404);
  const post = (path, body) => fetch(`${origin}/${path}`, { method: 'POST', body: JSON.stringify(body) });
  assert.equal((await post('wrong/tap', { x: 1, y: 2 })).status, 404);
  for (const [operation, body] of [
    ['tap', { x: '1; input keyevent HOME', y: 2 }],
    ['tap', { x: -1, y: 2 }],
    ['key', { key: 'HOME' }],
    ['swipe', { x1: 0, y1: 0, x2: 1, y2: 1, duration: 60000 }],
  ]) assert.equal((await post(`${token}/${operation}`, body)).status, 400);
  assert.deepEqual(calls, []);
  assert.equal((await post(`${token}/tap`, { x: 10, y: 20 })).status, 200);
  assert.deepEqual(calls, [['shell', 'input', 'tap', '10', '20']]);
});

test('a failed UI dump never returns an old hierarchy', async (t) => {
  const calls = [];
  const { server, token } = createAndroidDeviceFixture(async (args) => {
    calls.push(args);
    return 'ERROR: could not get idle state.';
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise((resolve) => { server.close(resolve); server.closeAllConnections(); }));
  const response = await fetch(`http://127.0.0.1:${server.address().port}/${token}/hierarchy`);
  assert.equal(response.status, 500);
  assert.match((await response.json()).error, /fresh hierarchy/);
  assert.equal(calls.length, 3);
  assert.ok(calls.every((args) => args[1] === 'uiautomator'));
});
