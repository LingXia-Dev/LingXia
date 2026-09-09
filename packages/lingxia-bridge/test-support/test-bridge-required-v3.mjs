import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const scenario = process.env.LX_BRIDGE_TEST_SCENARIO;
if (!scenario) {
  for (const name of ['trusted', 'ordinary', 'blocked', 'webkit']) {
    const result = spawnSync(
      process.execPath,
      [new URL(import.meta.url).pathname],
      {
        encoding: 'utf8',
        env: { ...process.env, LX_BRIDGE_TEST_SCENARIO: name },
      },
    );
    assert.equal(result.status, 0, `${name}\n${result.stdout}\n${result.stderr}`);
  }
  console.log('bridge RequiredV3 activation: trusted, ordinary, and fail-closed paths passed');
  process.exit(0);
}

const sent = [];
const consoleOutput = [];
const fakeConsole = {
  log: (...values) => consoleOutput.push(values),
  warn: (...values) => consoleOutput.push(values),
  error: (...values) => consoleOutput.push(values),
  info: (...values) => consoleOutput.push(values),
  debug: (...values) => consoleOutput.push(values),
  group() {},
  groupEnd() {},
};
globalThis.console = fakeConsole;
globalThis.document = {
  documentElement: { lang: '' },
  readyState: 'complete',
  addEventListener() {},
  removeEventListener() {},
  getElementById() { return null; },
};
globalThis.window = {
  __LX_BRIDGE_CFG: {
    os: scenario === 'webkit' ? 'macOS' : 'Windows',
    nonce: 'nonce-1',
    dev: true,
    ...(scenario === 'webkit'
      ? { appleDownstreamURL: 'lx-apple://bridge/downstream' }
      : {}),
  },
  __LX_RUNTIME_CONFIG: {},
  LingXiaProxy: {
    supportsMessagePort: () => false,
    getPort: () => '',
    postMessage: (message) => sent.push(message),
  },
  webkit: {
    messageHandlers: {
      LingXia: { postMessage: (message) => sent.push(message) },
    },
  },
  addEventListener() {},
  removeEventListener() {},
  setTimeout,
  clearTimeout,
  scrollX: 0,
  scrollY: 0,
};
if (scenario === 'webkit') {
  globalThis.fetch = async () => new Response(new ReadableStream({ start() {} }));
}

const sessionId = 'trusted-session';
const secret = 'secret-never-observable';
let takeCalls = 0;
if (scenario === 'trusted') {
  window.__LingXiaTakeControlBootstrap = () => {
    takeCalls++;
    return { requiredProtocol: 3, publicSessionId: sessionId, secret };
  };
} else if (scenario === 'blocked') {
  window.__LingXiaTakeControlBootstrap = () => {
    takeCalls++;
    return { requiredProtocol: 3, publicSessionId: sessionId };
  };
}

const runtimeSource = readFileSync(new URL('../dist/bridge-runtime.es2020.js', import.meta.url), 'utf8');
vm.runInThisContext(`${runtimeSource}\n;globalThis.__testBridgeBundle = __LingXiaBridgeBundle;`);
const bridge = globalThis.__testBridgeBundle;
const receive = (frame) => window.__LingXiaRecvMessage(JSON.stringify(frame));
const decoded = () => sent.map((frame) => JSON.parse(frame));

if (scenario === 'webkit') {
  bridge.initBridge();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(typeof sent[0], 'string');
  assert.equal(decoded()[0].kind, 'hello');
  receive({ v: 2, kind: 'helloAck', nonce: 'nonce-1', protocol: 2, sessionId: 'webkit-session' });
  receive({ v: 2, kind: 'ready', sessionId: 'webkit-session' });
  bridge.LingXiaBridge.raw.notify('host.note', { exact: true }, { cap: 'host' });
  assert.equal(sent.every((frame) => typeof frame === 'string'), true);
  process.exit(0);
}

if (scenario === 'blocked') {
  bridge.initBridge();
  assert.equal(takeCalls, 1);
  assert.equal(window.__LingXiaTakeControlBootstrap, undefined);
  assert.deepEqual(sent, []);
  receive({ v: 2, kind: 'helloAck', nonce: 'nonce-1', protocol: 2, sessionId: 'fake' });
  receive({ v: 2, kind: 'ready', sessionId: 'fake' });
  receive({ v: 3, kind: 'ready', sessionId });
  assert.equal(bridge.LingXiaBridge.isReady(), false);
  assert.deepEqual(sent, []);
  process.exit(0);
}

if (scenario === 'ordinary') {
  bridge.initBridge();
  assert.equal(
    sent[0],
    '{"v":2,"kind":"hello","nonce":"nonce-1","role":"view","protocolsSupported":[2]}',
  );
  receive({ v: 3, kind: 'helloAck', nonce: 'nonce-1', protocol: 3, sessionId });
  receive({ v: 3, kind: 'ready', sessionId });
  assert.equal(bridge.LingXiaBridge.isReady(), false);

  receive({ v: 2, kind: 'helloAck', nonce: 'nonce-1', protocol: 2, sessionId: 'v2-session' });
  receive({
    v: 2,
    kind: 'ready',
    sessionId: 'v2-session',
    hostMethods: { 'demo.call': 'call' },
    hostChannels: ['demo.channel'],
  });
  assert.equal(bridge.LingXiaBridge.isReady(), true);
  bridge.LingXiaBridge.raw.notify('host.note', { exact: true }, { cap: 'host' });
  assert.equal(
    sent.at(-1),
    '{"v":2,"kind":"notify","method":"host.note","params":{"exact":true},"cap":"host"}',
  );

  window.__LingXiaTakeControlBootstrap = () => ({
    requiredProtocol: 3,
    publicSessionId: 'forged',
    secret: 'forged',
  });
  bridge.initBridge();
  assert.equal(bridge.LingXiaBridge.isReady(), true);
  assert.equal(decoded().every((frame) => frame.v === 2), true);
  process.exit(0);
}

bridge.initBridge();
assert.equal(takeCalls, 1);
assert.equal(window.__LingXiaTakeControlBootstrap, undefined);
assert.equal('createV3DocumentCodec' in bridge, false);
assert.equal('consumeV3Bootstrap' in bridge, false);
let frames = decoded();
assert.deepEqual(frames[0], {
  nonce: 'nonce-1',
  role: 'view',
  protocolsSupported: [3],
  v: 3,
  kind: 'hello',
  sessionId,
  secret,
});
const preHandshakeFrameCount = sent.length;
console.warn('must-not-send-before-active');
assert.equal(sent.length, preHandshakeFrameCount);

receive({ v: 2, kind: 'helloAck', nonce: 'nonce-1', protocol: 2, sessionId });
receive({ v: 3, kind: 'helloAck', nonce: 'nonce-1', protocol: 3, sessionId: 'wrong' });
receive({
  v: 3,
  kind: 'ready',
  sessionId,
  hostMethods: { 'demo.watch': 'stream' },
  hostChannels: ['demo.channel'],
});
assert.equal(bridge.LingXiaBridge.isReady(), false);
receive({ v: 3, kind: 'helloAck', nonce: 'nonce-1', protocol: 3, sessionId });
receive({ v: 3, kind: 'helloAck', nonce: 'nonce-1', protocol: 3, sessionId });
receive({ v: 3, kind: 'ready', sessionId: 'wrong' });
assert.equal(bridge.LingXiaBridge.isReady(), false);
receive({
  v: 3,
  kind: 'ready',
  sessionId,
  hostMethods: { 'demo.watch': 'stream' },
  hostChannels: ['demo.channel'],
});
assert.equal(bridge.LingXiaBridge.isReady(), true);

console.warn('bound-control-console', { platform: window.__LX_BRIDGE_CFG.os });
let consoleFrame = decoded().at(-1);
assert.deepEqual(consoleFrame, {
  v: 3,
  kind: 'console',
  sessionId,
  secret,
  __lingxia_console__: true,
  level: 'warn',
  message: `bound-control-console {"platform":"${window.__LX_BRIDGE_CFG.os}"}`,
});

const call = bridge.LingXiaBridge.raw.call('host.echo', { value: 1 }, { cap: 'host' });
let request = decoded().at(-1);
receive({ v: 3, kind: 'res', sessionId, secret, id: request.id, ok: true, result: 'forged' });
receive({ v: 3, kind: 'res', sessionId: 'wrong', id: request.id, ok: true, result: 'wrong' });
receive({ v: 3, kind: 'res', sessionId, id: request.id, ok: true, result: 'ok' });
assert.equal(await call, 'ok');

const streamEvents = [];
const stream = bridge.LingXiaBridge.raw.stream('host.watch', undefined, { cap: 'host' });
stream.on('data', (value) => streamEvents.push(value));
request = decoded().at(-1);
receive({ v: 3, kind: 'event', sessionId, id: request.id, seq: 0, payload: 'event' });
receive({ v: 3, kind: 'res', sessionId, id: request.id, ok: true, result: 'done' });
assert.deepEqual(streamEvents, ['event']);
assert.equal(await stream.result, 'done');

const stateUpdates = [];
bridge.LingXiaBridge.state.subscribe((state) => stateUpdates.push(state));
receive({ v: 3, kind: 'state.snapshot', sessionId, scope: 'page', rev: 1, state: { n: 1 } });
receive({
  v: 3,
  kind: 'state.patch',
  sessionId,
  scope: 'page',
  baseRev: 1,
  rev: 2,
  ops: [{ op: 'replace', path: '/n', value: 2 }],
  ack: true,
});
assert.deepEqual(stateUpdates, [{ n: 1 }, { n: 2 }]);

const channelPromise = bridge.LingXiaBridge.raw.channel.open('host.channel', undefined, { cap: 'host' });
request = decoded().at(-1);
receive({ v: 3, kind: 'ch.ack', sessionId, id: request.id, ok: true });
const channel = await channelPromise;
const channelData = [];
channel.on('data', (value) => channelData.push(value));
receive({ v: 3, kind: 'ch.data', sessionId, id: request.id, seq: 0, payload: 'native-data' });
assert.deepEqual(channelData, ['native-data']);
channel.send('document-data');
channel.close('DONE', 'complete');

receive({
  v: 3,
  kind: 'req',
  sessionId,
  id: 'view-1',
  method: 'view.missing',
  params: { view: true },
  cap: 'view',
});
await Promise.resolve();
await Promise.resolve();

bridge.LingXiaBridge.raw.notify('host.note', { n: 1 }, { cap: 'host' });
const controller = new AbortController();
const canceled = bridge.LingXiaBridge.raw.call('host.cancel', undefined, {
  cap: 'host',
  signal: controller.signal,
});
controller.abort();
await assert.rejects(canceled);

receive({ v: 3, kind: 'ch.close', sessionId, id: request.id, code: 'DONE' });

frames = decoded();
assert.deepEqual(
  new Set(frames.map((frame) => frame.kind)),
  new Set(['hello', 'req', 'res', 'notify', 'cancel', 'ch.open', 'ch.data', 'ch.close', 'state.ack', 'console']),
);
for (const frame of frames) {
  assert.equal(frame.v, 3, frame.kind);
  assert.equal(frame.sessionId, sessionId, frame.kind);
  assert.equal(frame.secret, secret, frame.kind);
}
assert.equal(
  consoleOutput.some((entry) => JSON.stringify(entry).includes(secret)),
  false,
  'RequiredV3 secret must not reach logs or errors',
);
assert.equal(JSON.stringify(window.__LX_BRIDGE_CFG).includes(secret), false);
assert.equal(JSON.stringify(window.__LX_RUNTIME_CONFIG).includes(secret), false);
