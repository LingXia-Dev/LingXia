import { test } from 'node:test';
import assert from 'node:assert/strict';
import { checkDocument } from '@liulidesign/jinshu/schema';
import { demoRegistry, longDoc, meetingDoc } from '../shared/fixtures.ts';

test('the long document has 800 top-level blocks and no errors; the meeting notes are valid', () => {
  const long = longDoc();
  assert.equal(long.blocks.length, 800);
  for (const doc of [long, meetingDoc]) assert.deepEqual(checkDocument(doc, demoRegistry).filter(p => p.severity === 'error'), []);
});
