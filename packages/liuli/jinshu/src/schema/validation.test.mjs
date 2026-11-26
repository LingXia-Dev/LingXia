import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  createDocument,
  createRegistry,
  defineInline,
  migrateDocument,
  paragraph,
  parseAgentMarkup,
  parseInlineMarkup,
  validateJsonSchema,
  validateSrc,
} from './index';

describe('schema and validation rules', () => {
  it('a protocol-relative src names no scheme, so allowedSrcSchemes refuses it', () => {
    const https = { allowedSrcSchemes: ['https'] };
    assert.equal(validateSrc('https://cdn.example/x.png', https), null);
    assert.match(validateSrc('//evil.example/x.png', https), /protocol-relative/);
    assert.match(validateSrc('\\\\evil.example/x.png', https), /protocol-relative/);
    assert.equal(validateSrc('images/x.png', https), null);
  });

  it('an inline atom older than its spec is migrated, as a block is', () => {
    const chip = defineInline({
      inline: 'x-acme-chip',
      version: 2,
      attrsSchema: { type: 'object', properties: { label: { type: 'string' } }, required: ['label'] },
      migrate: (attrs, from) => (from === 1 ? { label: String(attrs.name) } : attrs),
      aiDescription: 'A chip.',
      fallbackText: attrs => attrs.label,
    });
    const registry = createRegistry({ inlines: [chip] });
    const old = createDocument({ blocks: [{ ...paragraph('', 'p'), runs: [{ inline: 'x-acme-chip', id: 'at1', version: 1, attrs: { name: 'Ana' } }] }] });
    const atom = migrateDocument(old, registry).blocks[0].runs[0];
    assert.deepEqual([atom.version, atom.attrs], [2, { label: 'Ana' }]);
    assert.deepEqual(old.blocks[0].runs[0].attrs, { name: 'Ana' }, 'the caller\'s document is left as it was');
  });

  it('a schema-valued additionalProperties checks the extra keys; inherited names are not properties', () => {
    const schema = { type: 'object', properties: { label: { type: 'string' } }, additionalProperties: { type: 'number' } };
    assert.deepEqual(validateJsonSchema({ label: 'a', width: 3 }, schema), []);
    assert.equal(validateJsonSchema({ label: 'a', width: 'wide' }, schema).length, 1);
    const needs = { type: 'object', required: ['toString', 'constructor'] };
    assert.equal(validateJsonSchema({}, needs).length, 2);
    const closed = { type: 'object', additionalProperties: false };
    assert.equal(validateJsonSchema({ any: 1 }, closed).length, 1);
  });

  it('an lx: link is kept when allowedLxApps names its app, and dropped otherwise', () => {
    const markup = '<p>See <a href="lx://notes/today">today</a>.</p>';
    const allowed = parseAgentMarkup(markup, { check: { allowedLxApps: ['notes'] } });
    assert.equal(allowed.blocks[0].runs.find(run => run.link)?.link, 'lx://notes/today');
    const refused = parseAgentMarkup(markup);
    assert.equal(refused.blocks[0].runs.some(run => run.link), false);
    const inline = parseInlineMarkup('<a href="lx://notes/today">today</a>', { check: { allowedLxApps: ['notes'] } });
    assert.equal(inline.runs[0].link, 'lx://notes/today');
  });
});
