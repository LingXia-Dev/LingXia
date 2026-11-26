import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  applyOps,
  columns,
  createDocument,
  createRegistry,
  defineBlock,
  paragraph,
  parseAgentMarkup,
  plainText,
  table,
} from './index';

const label = defineBlock({
  type: 'x-acme-label',
  version: 1,
  attrsSchema: { type: 'object' },
  content: 'inline',
  aiDescription: 'A label.',
  fallbackText: () => '',
});
const panel = defineBlock({
  type: 'x-acme-panel',
  version: 1,
  attrsSchema: { type: 'object' },
  content: { blocks: 'block+' },
  aiDescription: 'A panel.',
  fallbackText: () => '',
});
const registry = createRegistry({ blocks: [label, panel] });
const ok = res => assert.deepEqual(res.problems.filter(p => p.severity === 'error'), []);

describe('ops reach into extension blocks', () => {
  it('edits the inline content of an extension block', () => {
    const doc = createDocument({ blocks: [{ id: 'x', type: 'x-acme-label', version: 1, attrs: {}, fallback: '', runs: [{ text: 'Old name' }] }] });
    const res = applyOps(doc, [{ op: 'edit', id: 'x', find: 'Old', replace: 'New' }], registry);
    ok(res);
    assert.equal(plainText(res.doc.blocks[0].runs), 'New name');
  });

  it('inserts into a column inside an extension block', () => {
    const cols = columns([{ id: 'c1', blocks: [paragraph('Left.', 'l')] }, { id: 'c2', blocks: [paragraph('Right.', 'r')] }], { id: 'cols' });
    const doc = createDocument({
      blocks: [{ id: 'x', type: 'x-acme-panel', version: 1, attrs: {}, fallback: '', blocks: [cols] }],
    });
    const res = applyOps(doc, [{ op: 'insert', at: { end: 'c2' }, blocks: [paragraph('More.', 'm')] }], registry);
    ok(res);
    assert.deepEqual(res.doc.blocks[0].blocks[0].columns[1].blocks.map(b => b.id), ['r', 'm']);
  });

  it('a new column with an alignment or width gives a table without them one per column', () => {
    const doc = createDocument({ blocks: [table(['A', 'B'], [['1', '2']], { id: 't' })] });
    const res = applyOps(doc, [{ op: 'tableCol', id: 't', action: 'insert', index: 1, align: 'end', width: 2 }], registry);
    ok(res);
    assert.deepEqual(res.doc.blocks[0].align, ['start', 'end', 'start']);
    assert.deepEqual(res.doc.blocks[0].widths, [1, 2, 1]);
  });
});

describe('block code in markup', () => {
  it('an indented fence is a fence: its markers and indent are not part of the code', () => {
    const parsed = parseAgentMarkup('<quote>\n  <code language="js">\n    ```\n    let a = 1;\n      nested();\n    ```\n  </code>\n</quote>');
    const code = parsed.blocks[0].blocks[0];
    assert.equal(code.type, 'code');
    assert.equal(code.code, 'let a = 1;\n  nested();');
  });
});
