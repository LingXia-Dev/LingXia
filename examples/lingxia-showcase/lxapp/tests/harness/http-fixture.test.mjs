import assert from 'node:assert/strict';
import { after, before, test } from 'node:test';
import { createFixtureServer, digest } from './http-fixture.mjs';

let base;
let server;

before(async () => {
  server = createFixtureServer();
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  base = `http://127.0.0.1:${server.address().port}`;
});

after(() => server.close());

test('serves deterministic bytes with a matching digest', async () => {
  const response = await fetch(`${base}/bytes?size=4096`);
  const bytes = Buffer.from(await response.arrayBuffer());
  assert.equal(response.status, 200);
  assert.equal(bytes.length, 4096);
  assert.equal(response.headers.get('etag'), `"${digest(4096)}"`);

  const again = Buffer.from(await (await fetch(`${base}/bytes?size=4096`)).arrayBuffer());
  assert.ok(bytes.equals(again), 'the same request must produce the same bytes');
});

test('trickles a slow stream so progress is observable', async () => {
  const started = Date.now();
  const response = await fetch(`${base}/slow?size=4096&chunks=4&delayMs=40`);
  const bytes = Buffer.from(await response.arrayBuffer());
  assert.equal(bytes.length, 4096);
  assert.ok(Date.now() - started >= 120, 'the stream must not arrive at once');
});

test('truncates a body that promised more', async () => {
  await assert.rejects(async () => {
    const response = await fetch(`${base}/truncated?size=4096`);
    await response.arrayBuffer();
  });
});

test('answers any requested status', async () => {
  for (const code of [204, 404, 500, 503]) {
    assert.equal((await fetch(`${base}/status?code=${code}`)).status, code);
  }
});

test('reports the multipart parts it received', async () => {
  const form = new FormData();
  form.set('file', new Blob([Buffer.alloc(1024, 7)], { type: 'image/png' }), 'shot.png');
  form.set('note', 'from a spec');
  const response = await fetch(`${base}/upload`, {
    method: 'POST',
    body: form,
    headers: { 'x-lx-test': 'echo-me' },
  });

  assert.equal(response.status, 200);
  const body = await response.json();
  assert.equal(body.ok, true);
  assert.deepEqual(body.file, {
    field: 'file',
    filename: 'shot.png',
    type: 'image/png',
    bytes: 1024,
  });
  assert.deepEqual(body.fields, { note: 'from a spec' });
  assert.equal(body.headerEcho, 'echo-me');
  // The envelope carries the file plus its boundaries and headers.
  assert.ok(body.received > 1024, `envelope should exceed the file: ${body.received}`);
});

test('rejects a body that is not multipart', async () => {
  const response = await fetch(`${base}/upload`, {
    method: 'POST',
    body: 'plain',
    headers: { 'content-type': 'text/plain' },
  });
  assert.equal(response.status, 415);
});

test('stops writing when the client aborts mid-stream', async () => {
  const controller = new AbortController();
  const started = Date.now();
  // 20 chunks x 60ms is 1.2s if the handler ignores the abort.
  const request = fetch(`${base}/slow?size=20480&chunks=20&delayMs=60`, {
    signal: controller.signal,
  }).then(async (response) => {
    for await (const _chunk of response.body) controller.abort();
  });

  await assert.rejects(request, (error) => error.name === 'AbortError');
  // The handler must let go too, or `server.close()` waits on it.
  await new Promise((resolve) => setTimeout(resolve, 200));
  assert.ok(Date.now() - started < 1000, 'the handler kept streaming after the abort');
});
