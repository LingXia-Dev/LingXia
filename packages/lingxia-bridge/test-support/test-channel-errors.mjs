import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import { BRIDGE_ERROR } from '../dist/es2020/types.js';

// Run the shipped View bridge against a controlled transport. Device tests
// own real Logic channels; this owns deterministic handshake/error races.
const sent = [];
const context = vm.createContext({
  console: { log() {}, warn() {}, error() {}, info() {}, debug() {}, group() {}, groupEnd() {} },
  setTimeout, clearTimeout,
  document: {
    documentElement: { lang: '' }, readyState: 'complete',
    addEventListener() {}, removeEventListener() {}, getElementById() { return null; },
  },
  window: {
    __LX_BRIDGE_CFG: { os: 'Windows', nonce: 'channel-test', dev: true },
    __LX_RUNTIME_CONFIG: {},
    LingXiaProxy: {
      supportsMessagePort: () => false, getPort: () => '',
      postMessage: (frame) => sent.push(JSON.parse(frame)),
    },
    addEventListener() {}, removeEventListener() {}, setTimeout, clearTimeout,
    scrollX: 0, scrollY: 0,
  },
});
vm.runInContext(readFileSync(new URL('../dist/bridge-runtime.es2020.js', import.meta.url), 'utf8'), context);
const bundle = vm.runInContext('__LingXiaBridgeBundle', context);
const bridge = bundle.LingXiaBridge;
const receive = (frame) => context.window.__LingXiaRecvMessage(JSON.stringify({ v: 2, ...frame }));
const frames = (kind) => sent.filter((frame) => frame.kind === kind);
bundle.initBridge();
const opening = bridge.raw.channel.open('tickerSession');
assert.equal(frames('ch.open').length, 0, 'opening waits for ready');
receive({ kind: 'helloAck', nonce: 'channel-test', protocol: 2, sessionId: 'test-session' });
assert.equal(frames('ch.open').length, 0, 'helloAck alone is not ready');
receive({ kind: 'ready', sessionId: 'test-session' });
const request = frames('ch.open').at(-1);
assert.ok(request);
receive({ kind: 'ch.ack', id: request.id, ok: false,
  error: { code: 'BRIDGE_NOT_READY', message: 'Logic is starting', data: { retry: true } } });
await assert.rejects(opening, (error) => {
  assert.equal(error.code, 'BRIDGE_NOT_READY');
  assert.equal(error.message, 'Logic is starting');
  assert.equal(error.data.retry, true);
  return true;
});
// A late success must not resurrect the rejected handle.
receive({ kind: 'ch.ack', id: request.id, ok: true });
receive({ kind: 'ch.data', id: request.id, seq: 0, payload: 'stale' });

async function openChannel() {
  const pending = bridge.raw.channel.open('tickerSession');
  const { id } = frames('ch.open').at(-1);
  receive({ kind: 'ch.ack', id, ok: true });
  return pending;
}

for (const initiator of ['client', 'host']) {
  const channel = await openChannel();
  let closeCount = 0;
  const errors = [];
  const data = [];
  channel.on('close', () => closeCount++);
  channel.on('error', (error) => errors.push(error.code));
  channel.on('data', (value) => data.push(value));
  channel.send('before');
  assert.equal(frames('ch.data').at(-1).payload, 'before');
  const iterator = channel[Symbol.asyncIterator]();
  const read = iterator.next();
  const beforeClose = frames('ch.close').length;
  if (initiator === 'client') channel.close('done', 'finished');
  else receive({ kind: 'ch.close', id: channel.id, code: 'done', reason: 'finished' });
  assert.equal((await read).done, true, `${initiator} close settles a pending read`);
  channel.close();
  receive({ kind: 'ch.close', id: channel.id, code: 'duplicate' });
  receive({ kind: 'ch.data', id: channel.id, seq: 1, payload: 'late' });
  const beforeSend = frames('ch.data').length;
  channel.send('after');
  assert.deepEqual(errors, ['BRIDGE_STREAM_CLOSED']);
  assert.equal(frames('ch.data').length, beforeSend, 'closed send never reaches transport');
  assert.equal(frames('ch.close').length - beforeClose, initiator === 'client' ? 1 : 0);
  assert.equal(closeCount, 1);
  assert.deepEqual(data, []);
  assert.equal((await iterator.next()).done, true);
}

await assert.rejects(bridge.raw.channel.open(''), (error) => error.code === 'BRIDGE_MALFORMED_MESSAGE');
// Error-envelope ownership for the whole catalog. This verifies preservation,
// not that the client/host can physically produce every failure condition.
for (const code of Object.values(BRIDGE_ERROR)) {
  const opening = bridge.raw.channel.open('tickerSession');
  const { id } = frames('ch.open').at(-1);
  receive({ kind: 'ch.ack', id, ok: false, error: { code, message: `failure ${code}`, data: { detail: code } } });
  await assert.rejects(opening, (error) => {
    assert.equal(error.code, code);
    assert.equal(error.message, `failure ${code}`);
    assert.equal(error.data.detail, code);
    return true;
  });
}
console.log('channel errors: readiness, rejected open, close races, disconnected sends passed');
