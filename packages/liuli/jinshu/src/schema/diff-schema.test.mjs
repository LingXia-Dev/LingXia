import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { createDocument, diffDocuments, exportAiSchema, paragraph } from './index.ts';

const doc = (...pairs) => createDocument({ blocks: pairs.map(([id, text]) => paragraph(text, id)) });

describe('diffDocuments moves', () => {
  it('one block moving up is the only move', () => {
    const a = doc(['a', 'A'], ['b', 'B'], ['c', 'C'], ['d', 'D']);
    const b = doc(['d', 'D'], ['a', 'A'], ['b', 'B'], ['c', 'C']);
    const diff = diffDocuments(a, b);
    assert.equal(diff.stats.moved, 1);
    assert.deepEqual(diff.entries.filter(e => e.kind === 'moved').map(e => e.id), ['d']);
    assert.equal(diff.stats.unchanged, 3);
  });

  it('a block both moved and edited reports both', () => {
    const a = doc(['a', 'A'], ['b', 'B'], ['c', 'C']);
    const b = doc(['c', 'C edited'], ['a', 'A'], ['b', 'B']);
    const diff = diffDocuments(a, b);
    const c = diff.entries.find(e => e.id === 'c');
    assert.equal(c.kind, 'modified');
    assert.deepEqual(c.moved, { fromIndex: 2, toIndex: 0 });
    assert.equal(diff.stats.moved, 0, 'one entry, counted once');
    assert.equal(diff.stats.modified, 1);
  });
});

describe('jinshu_edit update attrs schema', () => {
  const attrs = () => {
    const update = exportAiSchema().operationsSchema.items.oneOf.find(s => s.properties?.op?.const === 'update');
    return update.properties.attrs.properties;
  };
  const accepts = (schema, kind) => (schema.anyOf ?? [schema]).some(s => (kind === 'null' ? s.type === 'null' : s.type === kind));

  it('a field name several block types share takes each of their types', () => {
    const title = attrs().title;
    assert.ok(accepts(title, 'string'), 'audio title is a string');
    assert.ok(accepts(title, 'array'), 'callout title is inlines');
  });

  it('an optional field accepts null, which removes it', () => {
    const { caption, alt } = attrs();
    assert.ok(accepts(caption, 'null'));
    assert.ok(accepts(alt, 'null'));
  });
});

describe('applyOps returns its ops with the ids it minted', () => {
  it('applying them again gives the same document', async () => {
    const { applyOps, table } = await import('./index.ts');
    const base = createDocument({ title: 'T', blocks: [paragraph('One', 'p1'), table(null, [['a']], { id: 't' })] });
    const cases = [
      [{ op: 'insert', at: { end: 'doc' }, markup: '<p>New</p><list><li>x</li></list>' }],
      [{ op: 'replace', id: 'p1', blocks: [{ type: 'paragraph', runs: [{ text: 'A' }] }, { type: 'quote', blocks: [{ type: 'paragraph', runs: [] }] }] }],
      [{ op: 'tableRow', id: 't', action: 'insert' }],
      [{ op: 'setType', id: 'p1', type: 'callout' }],
      [{ op: 'rewrite', markup: '<doc title="New"><p>One</p><p>Fresh</p></doc>' }],
    ];
    for (const ops of cases) {
      const first = applyOps(base, ops);
      assert.deepEqual(first.problems.filter(p => p.severity === 'error'), [], JSON.stringify(ops));
      assert.deepEqual(applyOps(base, first.ops).doc, first.doc, JSON.stringify(ops));
    }
  });
});
