import { test } from 'node:test';
import assert from 'node:assert/strict';
import { applyOps, checkDocument, normalizeDocument, parseAgentMarkup, toAgentView } from '@liulidesign/jinshu/schema';
import { demoRegistry, draftDoc, lessonDoc, meetingDoc } from '../shared/fixtures.ts';

const fixtures = { lessonDoc, draftDoc, meetingDoc };
const errors = problems => problems.filter(p => p.severity === 'error');

for (const [name, raw] of Object.entries(fixtures)) {
  test(`markup is lossless for ${name}`, () => {
    const doc = normalizeDocument(raw, demoRegistry);
    assert.deepEqual(errors(checkDocument(doc, demoRegistry)), []);
    const markup = toAgentView(doc, { registry: demoRegistry, maxChars: Infinity });
    const parsed = parseAgentMarkup(markup, { registry: demoRegistry });
    assert.deepEqual(errors(parsed.problems), []);
    const back = normalizeDocument(
      {
        version: parsed.version ?? 1,
        ...(parsed.id !== undefined ? { id: parsed.id } : {}),
        ...(parsed.title !== undefined ? { title: parsed.title } : {}),
        ...(parsed.lang !== undefined ? { lang: parsed.lang } : {}),
        ...(parsed.meta !== undefined ? { meta: parsed.meta } : {}),
        blocks: parsed.blocks,
      },
      demoRegistry,
    );
    assert.deepEqual(back, doc);
  });

  test(`rewrite with the unmodified agent view changes nothing for ${name}`, () => {
    const doc = normalizeDocument(raw, demoRegistry);
    const res = applyOps(doc, [{ op: 'rewrite', markup: toAgentView(doc, { registry: demoRegistry, maxChars: Infinity }) }], demoRegistry);
    assert.deepEqual(errors(res.problems), []);
    assert.deepEqual(res.doc, doc);
  });
}
