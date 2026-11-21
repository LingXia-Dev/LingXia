import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { after, before, test } from 'node:test';
import { createFixtureServer } from './http-fixture.mjs';

let base;
let server;

before(async () => {
  server = createFixtureServer();
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  base = `http://127.0.0.1:${server.address().port}`;
});

after(() => server.close());

test('serves the sample clip whole and by byte range', async () => {
  const clip = readFileSync(new URL('../fixtures/media/sample.mp4', import.meta.url));

  const whole = await fetch(`${base}/media/sample.mp4`);
  assert.equal(whole.status, 200);
  assert.equal(whole.headers.get('content-type'), 'video/mp4');
  assert.equal(whole.headers.get('accept-ranges'), 'bytes');
  assert.ok(Buffer.from(await whole.arrayBuffer()).equals(clip));

  const partial = await fetch(`${base}/media/sample.mp4`, { headers: { range: 'bytes=8-15' } });
  assert.equal(partial.status, 206);
  assert.equal(partial.headers.get('content-range'), `bytes 8-15/${clip.length}`);
  assert.ok(Buffer.from(await partial.arrayBuffer()).equals(clip.subarray(8, 16)));

  const tail = await fetch(`${base}/media/sample.mp4`, { headers: { range: 'bytes=1000-' } });
  assert.equal(tail.status, 206);
  assert.equal(Number(tail.headers.get('content-length')), clip.length - 1000);

  const beyond = await fetch(`${base}/media/sample.mp4`, { headers: { range: `bytes=${clip.length}-` } });
  assert.equal(beyond.status, 416);
});

test('serves the sample image as a real PNG', async () => {
  const png = readFileSync(new URL('../fixtures/media/sample.png', import.meta.url));
  const response = await fetch(`${base}/media/sample.png`);
  assert.equal(response.status, 200);
  assert.equal(response.headers.get('content-type'), 'image/png');
  const bytes = Buffer.from(await response.arrayBuffer());
  assert.ok(bytes.equals(png));
  // PNG magic, so a decoder — not just a byte comparison — accepts it.
  assert.deepEqual([...bytes.subarray(0, 4)], [0x89, 0x50, 0x4e, 0x47]);
});

test('serves a titled HTML page for browser-tab contracts', async () => {
  const response = await fetch(`${base}/page/tab-one`);
  assert.equal(response.status, 200);
  assert.match(response.headers.get('content-type'), /^text\/html/);
  const html = await response.text();
  assert.ok(html.includes('<title>fixture tab-one</title>'));
  assert.ok(html.includes('data-fixture-page="tab-one"'));
});
