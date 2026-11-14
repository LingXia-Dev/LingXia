import { test } from 'node:test';
import assert from 'node:assert/strict';
import { bmpPixels, createHarmonyDeviceFixture, layoutNodes } from './harmony-device-fixture.mjs';

async function serve(t, deps) {
  const { server, token } = createHarmonyDeviceFixture(deps);
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise((resolve) => { server.close(resolve); server.closeAllConnections(); }));
  return `http://127.0.0.1:${server.address().port}/${token}`;
}

// A 2x2 BMP, bottom-up, 24-bit: row 0 red then green, row 1 blue then white.
function bmp(signedHeight) {
  const stride = 8, offset = 54, raw = Buffer.alloc(offset + stride * 2);
  raw.write('BM', 0, 'latin1');
  raw.writeUInt32LE(offset, 10);
  raw.writeInt32LE(2, 18);
  raw.writeInt32LE(signedHeight, 22);
  raw.writeUInt16LE(24, 28);
  const rows = [[[255, 0, 0], [0, 255, 0]], [[0, 0, 255], [255, 255, 255]]];
  rows.forEach((row, y) => row.forEach(([r, g, b], x) => {
    const at = offset + (signedHeight > 0 ? 1 - y : y) * stride + x * 3;
    raw[at] = b; raw[at + 1] = g; raw[at + 2] = r;
  }));
  return raw;
}

test('pixels read a BMP either way up', () => {
  for (const height of [2, -2]) {
    assert.deepEqual(bmpPixels(bmp(height), [[0, 0], [1, 0], [0, 1], [1, 1]]),
      [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]]);
  }
  assert.throws(() => bmpPixels(bmp(2), [[2, 0]]), /outside/);
});

test('the layout flattens to text, id, type, description and pixel bounds', () => {
  const json = JSON.stringify({ attributes: { bounds: '[0,0][1080,2400]', type: 'root' }, children: [
    { attributes: { text: 'View lab', id: 'title', type: 'Text', description: 'Lab', bounds: '[400,120][680,180]' }, children: [] },
  ] });
  assert.deepEqual(layoutNodes(json), [
    { text: '', id: '', type: 'root', description: '', bounds: [0, 0, 1080, 2400] },
    { text: 'View lab', id: 'title', type: 'Text', description: 'Lab', bounds: [400, 120, 680, 180] },
  ]);
});

test('device input requires the capability URL and rejects shell-shaped arguments', async (t) => {
  const calls = [];
  const base = await serve(t, { hdc: async (args) => { calls.push(args); return ''; }, screen: async () => bmp(2) });
  const post = (path, body) => fetch(`${base}/${path}`, { method: 'POST', body: JSON.stringify(body) });
  assert.equal((await fetch(`${base.replace(/\/[^/]+$/, '/wrong')}/health`)).status, 404);
  for (const [operation, body] of [
    ['swipe', { x1: '0; reboot', y1: 0, x2: 1, y2: 1, velocity: 600 }],
    ['swipe', { x1: 0, y1: 0, x2: 1, y2: 1, velocity: 50 }],
    ['pixels', { points: [[0, -1]] }],
    ['key', { key: 'Back' }],
    ['longClick', { x: 1.5, y: 2 }],
    ['heldPixels', { x: 1, y: 1, points: [] }],
  ]) assert.equal((await post(operation, body)).status, 400);
  assert.deepEqual(calls, []);
  assert.equal((await post('swipe', { x1: 2, y1: 1200, x2: 540, y2: 1200, velocity: 1500 })).status, 200);
  assert.equal((await post('longClick', { x: 300, y: 900 })).status, 200);
  assert.deepEqual(calls, [['shell', 'uitest', 'uiInput', 'swipe', '2', '1200', '540', '1200', '1500'],
    ['shell', 'uitest', 'uiInput', 'longClick', '300', '900']]);
  assert.deepEqual((await (await post('pixels', { points: [[1, 1]] })).json()).pixels, [[255, 255, 255]]);
  assert.deepEqual((await (await post('heldPixels', { x: 1, y: 1, points: [[1, 1]] })).json()).pixels, [[255, 255, 255]]);
  assert.deepEqual(calls.at(-1), ['shell', 'uitest', 'uiInput', 'longClick', '1', '1']);
});

test('the layout dump is read and then removed from the device', async (t) => {
  const calls = [];
  const base = await serve(t, {
    hdc: async (args) => {
      calls.push(args[1]);
      return args[1] === 'cat' ? JSON.stringify({ attributes: { bounds: '[0,0][10,10]' } }) : '';
    },
    screen: async () => bmp(2),
  });
  const { nodes } = await (await fetch(`${base}/layout`)).json();
  assert.deepEqual(nodes, [{ text: '', id: '', type: '', description: '', bounds: [0, 0, 10, 10] }]);
  assert.deepEqual(calls, ['uitest', 'cat', 'rm']);
});
