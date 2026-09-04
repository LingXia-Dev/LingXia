import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import {
  DEFAULT_MAX_V3_FRAME_BYTES,
  createV3DocumentCodec,
} from '../../../target/lingxia-bridge-v3-test/protocol-v3.js';

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

for (const expected of invalidFixture.nativeToDocument) {
  assert.deepEqual(codec.parse(expected.frame), {
    ok: false,
    error: expected.error,
  });
}
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

const nested = invalidFixture.nestedSecurityKeysAreNotTopLevelDuplicates;
assert.equal(codec.parse(nested.nativeToDocument).ok, true);

const [indexSource, runtimeSource, es2020Bundle, es5Bundle] = await Promise.all([
  readFile(new URL('../src/index.ts', import.meta.url), 'utf8'),
  readFile(new URL('../src/bridge.ts', import.meta.url), 'utf8'),
  readFile(new URL('../dist/bridge-runtime.es2020.js', import.meta.url), 'utf8'),
  readFile(new URL('../dist/bridge-runtime.es5.js', import.meta.url), 'utf8'),
]);
assert.doesNotMatch(indexSource, /protocol-v3/);
assert.doesNotMatch(runtimeSource, /protocol-v3/);
assert.match(runtimeSource, /protocolsSupported:\s*\[2\]/);
assert.doesNotMatch(es2020Bundle, /V3_PROTOCOL|protocol-v3/);
assert.doesNotMatch(es5Bundle, /V3_PROTOCOL|protocol-v3/);

console.log(`bridge-v3 codec: ${fixture.inbound.length + fixture.outbound.length} golden frames passed`);
