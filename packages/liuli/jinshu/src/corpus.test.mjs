// Conformance corpus: src/testing/corpus. valid/ holds canonical JSON with its markup and, where Markdown
// can hold it, its Markdown; invalid/ documents with the errors they must raise; markdown/ hand-written
// Markdown with the document it reads as and the file Jinshu writes back.
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readdirSync, readFileSync } from 'node:fs';

import {
  JINSHU_DOC_VERSION,
  checkDocument,
  documentToMarkdown,
  markdownToDocument,
  normalizeDocument,
  parseAgentMarkup,
  toAgentView,
} from './schema/index.ts';

const root = new URL('./testing/corpus/', import.meta.url);
const read = path => readFileSync(new URL(path, root), 'utf8');
const names = (dir, ext) =>
  readdirSync(new URL(dir, root))
    .filter(f => f.endsWith(ext) && !f.endsWith(`.out${ext}`) && f !== 'expect.json')
    .map(f => f.slice(0, -ext.length))
    .sort();

/** The document a markup file reads as, the way the agent tools build it. */
function markupDoc(text) {
  const parsed = parseAgentMarkup(text);
  assert.deepEqual(parsed.problems, [], 'markup parses cleanly');
  return normalizeDocument({
    version: parsed.version ?? JINSHU_DOC_VERSION,
    ...(parsed.id !== undefined ? { id: parsed.id } : {}),
    ...(parsed.title !== undefined ? { title: parsed.title } : {}),
    ...(parsed.lang !== undefined ? { lang: parsed.lang } : {}),
    ...(parsed.meta !== undefined ? { meta: parsed.meta } : {}),
    blocks: [...parsed.blocks],
  });
}

const stripIds = value =>
  Array.isArray(value)
    ? value.map(stripIds)
    : value && typeof value === 'object'
      ? Object.fromEntries(Object.entries(value).filter(([k]) => k !== 'id').map(([k, v]) => [k, stripIds(v)]))
      : value;

describe('corpus: valid documents', () => {
  const valid = names('valid/', '.json');
  it('has cases', () => assert.ok(valid.length >= 5));

  for (const name of valid) {
    describe(name, () => {
      const json = read(`valid/${name}.json`);
      const doc = JSON.parse(json);

      it('validates with no problems and is stored canonical', () => {
        assert.deepEqual(checkDocument(doc), []);
        assert.equal(`${JSON.stringify(normalizeDocument(doc), null, 2)}\n`, json);
      });

      it('JSON ↔ markup', () => {
        const markup = read(`valid/${name}.xml`);
        assert.equal(`${toAgentView(doc)}\n`, markup);
        assert.deepEqual(markupDoc(markup), doc);
      });

      const mdPath = `valid/${name}.md`;
      if (existsSync(new URL(mdPath, root))) {
        it('JSON ↔ Markdown', () => {
          const md = read(mdPath);
          const written = documentToMarkdown(doc);
          assert.deepEqual(written.problems, []);
          assert.equal(written.text, md);
          // With the previous version every id survives; without it, only ids differ.
          const again = markdownToDocument(md, { previous: doc });
          assert.deepEqual(again.problems, []);
          assert.deepEqual(again.doc, doc);
          const fresh = markdownToDocument(md);
          assert.deepEqual(stripIds(fresh.doc), stripIds(doc));
          assert.equal(documentToMarkdown(fresh.doc).text, md);
          assert.deepEqual(markdownToDocument(md).doc, fresh.doc, 'derived ids are deterministic');
        });
      } else {
        it('is refused by the Markdown profile', () => {
          assert.ok(checkDocument(doc, undefined, { format: 'markdown' }).some(p => p.code === 'FORMAT' && p.severity === 'error'));
        });
      }
    });
  }
});

describe('corpus: invalid documents', () => {
  const expect = JSON.parse(read('invalid/expect.json'));
  const invalid = names('invalid/', '.json');
  it('every case has its expectation', () => assert.deepEqual(invalid, Object.keys(expect).sort()));

  for (const name of invalid) {
    it(name, () => {
      const doc = JSON.parse(read(`invalid/${name}.json`));
      const wanted = expect[name];
      const profile = wanted.some(w => w.profile === 'markdown') ? { format: 'markdown' } : undefined;
      if (profile) assert.deepEqual(checkDocument(doc).filter(p => p.severity === 'error'), [], 'valid as plain JSON');
      const errors = checkDocument(doc, undefined, profile).filter(p => p.severity === 'error');
      assert.deepEqual(
        errors.map(p => [p.code, p.id]),
        wanted.map(w => [w.code, w.id]),
      );
      wanted.forEach((w, i) => assert.ok(errors[i].message.includes(w.match), `${errors[i].message} ⊇ ${w.match}`));
    });
  }
});

describe('corpus: Markdown read and written back', () => {
  const expect = JSON.parse(read('markdown/expect.json'));
  const cases = names('markdown/', '.md');
  it('every case has its expectation', () => assert.deepEqual(cases, Object.keys(expect).sort()));

  for (const name of cases) {
    it(name, () => {
      const input = read(`markdown/${name}.md`);
      const parsed = markdownToDocument(input);
      assert.deepEqual(parsed.problems.map(p => [p.severity, p.subject?.rule]), expect[name].warnings.map(r => ['warning', r]));
      assert.equal(`${toAgentView(parsed.doc)}\n`, read(`markdown/${name}.xml`));
      assert.deepEqual(checkDocument(parsed.doc, undefined, { format: 'markdown' }).filter(p => p.severity === 'error'), []);
      // Unchanged, the file keeps its bytes; written fresh, it is the canonical form, which is a fixpoint.
      assert.equal(documentToMarkdown(parsed.doc, { source: parsed.source }).text, input);
      const out = read(`markdown/${name}.out.md`);
      assert.equal(documentToMarkdown(parsed.doc).text, out);
      const reread = markdownToDocument(out, { previous: parsed.doc });
      assert.deepEqual(reread.doc, parsed.doc);
      assert.equal(documentToMarkdown(reread.doc).text, out);
    });
  }
});
