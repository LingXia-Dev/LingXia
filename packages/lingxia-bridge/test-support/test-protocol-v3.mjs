import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import {
  DEFAULT_MAX_V3_FRAME_BYTES,
  createV3DocumentCodec,
  consumeV3Bootstrap,
} from '../../../target/lingxia-bridge-v3-test/src/protocol-v3.js';

const fixtureUrl = new URL('../../../testdata/bridge-v3/golden.json', import.meta.url);
const invalidFixtureUrl = new URL('../../../testdata/bridge-v3/invalid.json', import.meta.url);
const [fixture, invalidFixture] = await Promise.all(
  [fixtureUrl, invalidFixtureUrl].map(async (url) => JSON.parse(await readFile(url, 'utf8'))),
);
const codecResult = createV3DocumentCodec({
  sessionId: 'v3-session',
  secret: 'bridge-v3-test-secret',
});
assert.equal(codecResult.ok, true);
const codec = codecResult.value;

for (const expected of fixture.inbound) {
  const { v, kind, sessionId, secret, ...payload } = expected.frame;
  assert.equal(v, 3);
  assert.equal(kind, expected.kind);
  assert.equal(sessionId, 'v3-session');
  assert.equal(secret, 'bridge-v3-test-secret');
  const encoded = codec.encode(expected.kind, payload);
  assert.equal(encoded.ok, true, expected.kind);
  assert.deepEqual(encoded.value, expected.frame, expected.kind);
}

for (const expected of fixture.outbound) {
  const parsed = codec.parse(JSON.stringify(expected.frame));
  assert.equal(parsed.ok, true, expected.kind);
  assert.equal(parsed.value.kind, expected.kind);
  assert.deepEqual(parsed.value.payload, expected.payload, expected.kind);
  assert.equal(Object.hasOwn(parsed.value, 'secret'), false);
  assert.equal(Object.hasOwn(parsed.value.payload, 'secret'), false);
}

for (const expected of fixture.outbound) {
  assert.deepEqual(
    codec.parse(JSON.stringify({ ...expected.frame, sessionId: 'wrong-session' })),
    { ok: false, error: 'SESSION_MISMATCH' },
    `${expected.kind} must be session-bound before dispatch`,
  );
  assert.deepEqual(
    codec.parse(JSON.stringify({ ...expected.frame, secret: 'native-must-not-send-secret' })),
    { ok: false, error: 'UNEXPECTED_SECRET' },
    `${expected.kind} must reject a native-supplied secret`,
  );
}

for (const expected of invalidFixture.nativeToDocument) {
  assert.deepEqual(codec.parse(expected.frame), {
    ok: false,
    error: expected.error,
  });
}

assert.deepEqual(
  codec.parse('{"v":3,"kind":"helloAck","kind":"helloAck","sessionId":"v3-session"}'),
  { ok: false, error: 'MALFORMED_ENVELOPE' },
  'duplicate hello envelope keys reject before helloAck payload handling',
);
assert.deepEqual(
  codec.encode('hello', { sessionId: 'forged' }),
  { ok: false, error: 'SECURITY_FIELD_IN_PAYLOAD' },
  'a document hello cannot shadow its bound session envelope',
);
assert.deepEqual(
  codec.parse(JSON.stringify({ v: 2, kind: 'helloAck', sessionId: 'v3-session' })),
  { ok: false, error: 'UNSUPPORTED_VERSION' },
  'a V2 frame cannot downgrade a bound V3 codec',
);
assert.deepEqual(
  codec.parse(
    JSON.stringify({ v: 3, kind: 'ready', sessionId: 'v3-session', pad: 'a'.repeat(32) }),
    16,
  ),
  { ok: false, error: 'FRAME_TOO_LARGE' },
);
assert.equal(DEFAULT_MAX_V3_FRAME_BYTES, 64 * 1024);

for (const field of ['v', 'kind', 'sessionId', 'secret']) {
  assert.deepEqual(
    codec.encode('req', { [field]: 'forged' }),
    { ok: false, error: 'SECURITY_FIELD_IN_PAYLOAD' },
  );
}
for (const expected of invalidFixture.documentBindings) {
  assert.deepEqual(createV3DocumentCodec(expected.binding), {
    ok: false,
    error: expected.error,
  });
}
assert.deepEqual(codec.encode('req', null), { ok: false, error: 'INVALID_DOCUMENT_PAYLOAD' });
assert.deepEqual(
  codec.encode('console', {
    __lingxia_console__: true,
    level: 'warn',
    message: 'bound only',
  }),
  {
    ok: true,
    value: {
      v: 3,
      kind: 'console',
      sessionId: 'v3-session',
      secret: 'bridge-v3-test-secret',
      __lingxia_console__: true,
      level: 'warn',
      message: 'bound only',
    },
  },
);

const nested = invalidFixture.nestedSecurityKeysAreNotTopLevelDuplicates;
assert.equal(codec.parse(nested.nativeToDocument).ok, true);

const originalWindow = globalThis.window;
const bootstrapSecret = 'must-never-be-global';
let bootstrapCalls = 0;
const bootstrapWindow = {
  __LX_BRIDGE_CFG: { nonce: 'v2-nonce' },
  __LX_RUNTIME_CONFIG: {},
  __LingXiaTakeControlBootstrap() {
    bootstrapCalls++;
    return {
      requiredProtocol: 3,
      publicSessionId: 'private-v3-session',
      secret: bootstrapSecret,
    };
  },
};
globalThis.window = bootstrapWindow;
try {
  const activation = consumeV3Bootstrap();
  assert.equal(activation.kind, 'required');
  const bootstrapCodec = activation.codec;
  assert.equal(bootstrapCalls, 1);
  assert.equal(Object.hasOwn(bootstrapWindow, '__LingXiaTakeControlBootstrap'), false);
  assert.deepEqual(consumeV3Bootstrap(), { kind: 'absent' });
  assert.equal(bootstrapCalls, 1);
  assert.equal(JSON.stringify(bootstrapWindow).includes(bootstrapSecret), false);
  assert.equal(JSON.stringify(bootstrapCodec).includes(bootstrapSecret), false);
  assert.deepEqual(Object.keys(bootstrapCodec).sort(), ['encode', 'parse']);
  assert.deepEqual(
    bootstrapCodec.encode('hello', { nonce: 'v3-nonce', role: 'view', protocolsSupported: [3] }),
    {
      ok: true,
      value: {
        v: 3,
        kind: 'hello',
        sessionId: 'private-v3-session',
        secret: bootstrapSecret,
        nonce: 'v3-nonce',
        role: 'view',
        protocolsSupported: [3],
      },
    },
  );
} finally {
  globalThis.window = originalWindow;
}

const [indexSource, runtimeSource, es2020Bundle, es5Bundle] = await Promise.all([
  readFile(new URL('../src/index.ts', import.meta.url), 'utf8'),
  readFile(new URL('../src/bridge.ts', import.meta.url), 'utf8'),
  readFile(new URL('../dist/bridge-runtime.es2020.js', import.meta.url), 'utf8'),
  readFile(new URL('../dist/bridge-runtime.es5.js', import.meta.url), 'utf8'),
]);
assert.doesNotMatch(indexSource, /protocol-v3/);
assert.match(runtimeSource, /protocolMode\.kind === "required-v3" \? \[3\] : \[2\]/);
assert.match(runtimeSource, /consumeV3Bootstrap/);
assert.match(es2020Bundle, /__LingXiaTakeControlBootstrap/);
assert.match(es5Bundle, /__LingXiaTakeControlBootstrap/);

console.log(`bridge-v3 codec: ${fixture.inbound.length + fixture.outbound.length} golden frames passed`);
