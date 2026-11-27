import { test } from 'node:test';
import assert from 'node:assert/strict';
import { newReadingState, readVisibility } from './reading.ts';
import { revealTargetId } from './nav.ts';
import { createDocument, heading, inlineAtom, list, listItem, paragraph, run, table } from '../schema/index.ts';

const doc = createDocument({ blocks: [heading(2, 'A', 'h1'), paragraph('One.', 'p1'), heading(2, 'B', 'h2'), paragraph('Two.', 'p2')] });

test('readVisibility: dwell when a block leaves or has gathered a second, with the longest still stretch', () => {
  const state = newReadingState();
  let out = readVisibility(state, doc, 0, [{ id: 'h1', ratio: 1, top: 0 }, { id: 'p1', ratio: 0.5, top: 300 }]);
  assert.deepEqual(out.facts, [{ kind: 'position', blockId: 'h1', offset: 0 }]);
  assert.equal(out.headingId, 'h1');
  // A quick scroll: nothing has a second yet.
  out = readVisibility(state, doc, 400, [{ id: 'h1', ratio: 1, top: 0 }, { id: 'p1', ratio: 1, top: 200 }]);
  assert.deepEqual(out.facts, []);
  // Read for five seconds, then scroll h1 away.
  out = readVisibility(state, doc, 5400, [{ id: 'p1', ratio: 1, top: -10 }, { id: 'h2', ratio: 1, top: 200 }]);
  assert.deepEqual(out.left, ['h1']);
  assert.deepEqual(out.facts.filter(f => f.kind === 'dwell'), [
    { kind: 'dwell', blockId: 'h1', dwellMs: 5400, seenRatio: 1, idleMs: 5000 },
    { kind: 'dwell', blockId: 'p1', dwellMs: 5400, seenRatio: 1, idleMs: 5000 },
  ]);
  assert.deepEqual(out.facts.find(f => f.kind === 'position'), { kind: 'position', blockId: 'p1', offset: 10 });
  assert.equal(out.headingId, 'h1', 'the heading above the block being read');
  // p1 reported its time and keeps counting from here.
  out = readVisibility(state, doc, 5600, [{ id: 'h2', ratio: 1, top: 0 }]);
  assert.deepEqual(out.facts.filter(f => f.kind === 'dwell'), [{ kind: 'dwell', blockId: 'p1', dwellMs: 200, seenRatio: 1, idleMs: 200 }]);
  assert.equal(out.headingId, 'h2');
});

test('revealTargetId: items, rows and blocks are their own target; an atom resolves to what holds it', () => {
  const linked = createDocument({
    blocks: [
      list('bullet', [listItem('Item', { id: 'i1' })], { id: 'l1' }),
      table(['A'], [['1']], { id: 't1' }),
      paragraph([run('See '), inlineAtom('x-app-ref', { to: 1 }, 'ref', { id: 'a1' })], 'p1'),
    ],
  });
  assert.equal(revealTargetId(linked, 'i1'), 'i1');
  assert.equal(revealTargetId(linked, linked.blocks[1].rows[0].id), linked.blocks[1].rows[0].id);
  assert.equal(revealTargetId(linked, 'a1'), 'p1');
  assert.equal(revealTargetId(linked, 'nope'), undefined);
});
