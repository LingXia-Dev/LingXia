import test from 'node:test';
import assert from 'node:assert/strict';
import { readSchema, drift, generate } from '../contracts/generate.mjs';

test('committed artifacts match a fresh generation from the contract schema', async () => {
  assert.deepEqual(await drift(await readSchema()), []);
});

test('a schema change is reported as drift until the artifacts are regenerated', async () => {
  const schema = await readSchema();
  const changed = structuredClone(schema);
  changed.fields.push({ id: schema.fields.length, key: 'driftProbe', wire: 'driftProbe', kind: 'number' });
  const stale = await drift(changed);
  assert.ok(stale.includes('crates/lingxia-ui/src/protocol/generated.rs'));
  assert.ok(stale.includes('packages/lingxia-ui/src/generated/wire.js'));
  const removed = structuredClone(schema);
  removed.tags.pop();
  assert.ok((await drift(removed)).includes('crates/lingxia-ui/src/protocol/generated.rs'));
});

test('the schema rejects reused field keys and sparse ids', async () => {
  const schema = await readSchema();
  const reused = structuredClone(schema);
  reused.fields[1].key = reused.fields[0].key;
  await assert.rejects(generate(reused), /duplicate field key/);
  const sparse = structuredClone(schema);
  sparse.fields[1].id = 7;
  await assert.rejects(generate(sparse), /dense/);
});
