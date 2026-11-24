import test from 'node:test';
import assert from 'node:assert/strict';
import { readSchema, drift, generate } from '../../../contracts/view/generate.mjs';
import { errors, raisedCodesInJs } from '../../../contracts/view/docs.mjs';

test('committed artifacts match a fresh generation from the contract schema', async () => {
  assert.deepEqual(await drift(await readSchema()), []);
});

test('a schema change is reported as drift until the artifacts are regenerated', async () => {
  const schema = await readSchema();
  const changed = structuredClone(schema);
  changed.fields.push({ id: schema.fields.length, key: 'driftProbe', wire: 'driftProbe', kind: 'number' });
  const stale = await drift(changed);
  assert.ok(stale.includes('crates/lingxia-view/src/protocol/generated.rs'));
  assert.ok(stale.includes('packages/lingxia-view/src/generated/wire.js'));
  const added = structuredClone(schema);
  added.tags.push('drift-probe');
  added.admission['drift-probe'] = { children: 'none' };
  assert.ok((await drift(added)).includes('crates/lingxia-view/src/protocol/generated.rs'));
});

test('limits, admission and the CSS table reach native decode and the JS runtime together', async () => {
  const schema = await readSchema();
  const limit = structuredClone(schema);
  limit.limits.sourceBytes += 1;
  for (const path of ['crates/lingxia-view/src/protocol/generated.rs', 'crates/lingxia-lxapp/src/page/view_limits.rs', 'packages/lingxia-view/src/generated/contract.js'])
    assert.ok((await drift(limit)).includes(path), path);
  const nesting = structuredClone(schema);
  nesting.admission.text.children = 'any';
  const fieldTags = structuredClone(schema);
  fieldTags.fields.find(field => field.key === 'onClick').tags.push('scroll');
  const css = structuredClone(schema);
  css.style.width.css = 'inline-size';
  for (const changed of [nesting, fieldTags]) {
    const stale = await drift(changed);
    assert.ok(stale.includes('crates/lingxia-view/src/protocol/generated.rs'));
    assert.ok(stale.includes('packages/lingxia-view/src/generated/contract.js'));
  }
  assert.ok((await drift(css)).includes('packages/lingxia-view/src/generated/contract.js'));
});

test('the schema rejects incomplete admission and conflicting CSS longhands', async () => {
  const schema = await readSchema();
  const missing = structuredClone(schema);
  delete missing.admission.page;
  await assert.rejects(generate(missing), /admission must list every tag/);
  const unknown = structuredClone(schema);
  unknown.fields.find(field => field.key === 'onClick').tags = ['paragraph'];
  await assert.rejects(generate(unknown), /unknown tag paragraph/);
  const limit = structuredClone(schema);
  limit.limits.nodes = 0;
  await assert.rejects(generate(limit), /limit nodes must be a positive integer/);
  const twice = structuredClone(schema);
  twice.style.height.css = 'width';
  await assert.rejects(generate(twice), /css width sets both/);
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
  for (const path of ['packages/lingxia-view/src/generated/contract.js', 'packages/lingxia-view/src/generated/authoring.d.ts', 'crates/lingxia-view/src/session/tests/runtime.js'])
    assert.ok(stale.includes(path), path);
});

test('error code reference is generated from schema.errors, ignores prose mentions, and rejects stage or code drift', async () => {
  assert.deepEqual([...raisedCodesInJs(`
    // UI_TIMEOUT in comment
    /* UI_OVERRUN in block comment */
    const note = "drops the frame (UI_TIMEOUT after several) or see UI_BUDGET: info";
    const err = Error('UI_PROP: bad');
    const tpl = \`\${loc}: UI_CLASS: dynamic\`;
    const arg = label(v, 'UI_PLOT', true);
  `)].sort(), ['UI_CLASS', 'UI_PLOT', 'UI_PROP']);

  const schema = await readSchema();
  const errorsPage = await errors(schema);
  assert.match(errorsPage, /\| Code \| Raised at \| Severity \| When \| Fix \|/);
  assert.match(errorsPage, /\| `UI_STYLE_VALUE` \| build, runtime, native \| error \|/);
  assert.match(errorsPage, /\| `UI_VECTOR` \| build, runtime, native \| error \|/);
  assert.match(errorsPage, /\| `UI_MODULE` \| build, native \| error \|/);

  const missingStage = structuredClone(schema);
  missingStage.errors.UI_VECTOR.stages = ['runtime', 'native'];
  await assert.rejects(errors(missingStage), /UI_VECTOR is raised at build/);

  const extraStage = structuredClone(schema);
  extraStage.errors.UI_ACTION.stages = ['build', 'runtime', 'native'];
  await assert.rejects(errors(extraStage), /UI_ACTION \(build\)/);

  const missingCode = structuredClone(schema);
  delete missingCode.errors.UI_CLASS;
  await assert.rejects(errors(missingCode), /missing from schema\.json: UI_CLASS/);

  const staleJsOnly = structuredClone(schema);
  staleJsOnly.errors.UI_GHOST = {
    stages: ['build'],
    severity: 'error',
    when: 'Never raised.',
    fix: 'Remove it.',
  };
  await assert.rejects(errors(staleJsOnly), /UI_GHOST \(build\)/);
});
