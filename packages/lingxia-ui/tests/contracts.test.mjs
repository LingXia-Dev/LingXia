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
  const added = structuredClone(schema);
  added.tags.push('drift-probe');
  assert.ok((await drift(added)).includes('crates/lingxia-ui/src/protocol/generated.rs'));
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

test('author styles and elements must lower to fields and groups the schema defines', async () => {
  const schema = await readSchema();
  const unknownField = structuredClone(schema);
  unknownField.style.width.wire = 'missingField';
  await assert.rejects(generate(unknownField), /style width lowers to unknown field missingField/);
  const unknownGroup = structuredClone(schema);
  unknownGroup.elements.text.style.push('missing');
  await assert.rejects(generate(unknownGroup), /unknown style group missing/);
  const unknownTag = structuredClone(schema);
  unknownTag.elements.text.wire = 'paragraph';
  await assert.rejects(generate(unknownTag), /unknown tag paragraph/);
});

test('element and style tables reach the authoring types and runtime admission together', async () => {
  const schema = await readSchema();
  const changed = structuredClone(schema);
  changed.elements.text.style = changed.elements.text.style.filter(group => group !== 'textBlock');
  const stale = await drift(changed);
  for (const path of ['packages/lingxia-ui/src/generated/contract.js', 'packages/lingxia-ui/src/generated/authoring.d.ts', 'crates/lingxia-ui/src/session/tests/runtime.js'])
    assert.ok(stale.includes(path), path);
});
