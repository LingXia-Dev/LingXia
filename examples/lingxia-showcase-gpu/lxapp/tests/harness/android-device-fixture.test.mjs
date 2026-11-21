import { test } from 'node:test';
import assert from 'node:assert/strict';
import { request } from 'node:http';
import { createAndroidDeviceFixture, screenPixels } from './android-device-fixture.mjs';

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

test('rotate turns auto-rotate off and sets the user rotation, and restore puts back what it found', async (t) => {
  const calls = [];
  const settings = { accelerometer_rotation: '0', user_rotation: '3' };
  const { server, token } = createAndroidDeviceFixture(async (args) => {
    calls.push(args);
    return args[2] === 'get' ? `${settings[args[4]]}\n` : '';
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise((resolve) => { server.close(resolve); server.closeAllConnections(); }));
  const post = (body) => fetch(`http://127.0.0.1:${server.address().port}/${token}/rotate`, { method: 'POST', body: JSON.stringify(body) });
  assert.equal((await post({ orientation: 'sideways; reboot' })).status, 400);
  assert.equal((await post({ orientation: 'auto' })).status, 400, 'no turn forces auto-rotate on');
  assert.equal((await post({ orientation: 'landscape' })).status, 200);
  assert.equal((await post({ orientation: 'portrait' })).status, 200);
  assert.equal((await post({ orientation: 'restore' })).status, 200);
  assert.equal((await post({ orientation: 'restore' })).status, 200);
  const put = (name, value) => ['shell', 'settings', 'put', 'system', name, value];
  assert.deepEqual(calls, [
    ['shell', 'settings', 'get', 'system', 'accelerometer_rotation'],
    ['shell', 'settings', 'get', 'system', 'user_rotation'],
    put('accelerometer_rotation', '0'), put('user_rotation', '1'),
    // The second turn keeps the first record: the phone's own settings, not the fixture's.
    put('accelerometer_rotation', '0'), put('user_rotation', '0'),
    put('user_rotation', '3'), put('accelerometer_rotation', '0'),
  ], 'a locked phone stays locked; a second restore writes nothing');
});

test('pixels reads the composited screen at the given points, whichever header the device writes', async (t) => {
  const screen = (header) => {
    const raw = Buffer.alloc(header + 2 * 2 * 4);
    raw.writeUInt32LE(2, 0); raw.writeUInt32LE(2, 4);
    raw.set([10, 20, 30, 255], header + (1 * 2 + 1) * 4);
    return raw;
  };
  for (const header of [12, 16]) assert.deepEqual(screenPixels(screen(header), [[1, 1], [0, 0]]), [[10, 20, 30], [0, 0, 0]]);
  assert.throws(() => screenPixels(screen(12), [[2, 0]]), /outside/);
  const calls = [];
  const { server, token } = createAndroidDeviceFixture(async (args, options) => { calls.push([args, options]); return screen(16); });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise((resolve) => { server.close(resolve); server.closeAllConnections(); }));
  const post = (body) => fetch(`http://127.0.0.1:${server.address().port}/${token}/pixels`, { method: 'POST', body: JSON.stringify(body) });
  assert.equal((await post({ points: [['1; reboot', 0]] })).status, 400);
  const response = await post({ points: [[1, 1]] });
  assert.deepEqual(await response.json(), { pixels: [[10, 20, 30]] });
  assert.deepEqual(calls, [[['exec-out', 'screencap'], { binary: true }]]);
});
