import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { createDocument, heading, inlineMath, paragraph, plainText, quote } from '../schema/index.ts';
import { createJinshuSource, createJinshuSources } from './index.ts';
import { anySignal } from './signal.ts';

const HUMAN = { kind: 'human', id: 'editor' };
const AGENT = { kind: 'agent', id: 'helper' };
const wait = ms => new Promise(resolve => setTimeout(resolve, ms));
const lesson = () => createDocument({ title: 'W', blocks: [heading(1, 'W', 'h'), paragraph('First.', 'p1'), quote([paragraph('Inner.', 'q1')], 'q')] });

describe('the source model', () => {
  it('a range suggestion past math counts it as one place, and keeps the replacement\'s marks', () => {
    const doc = createDocument({ blocks: [{ ...paragraph('', 'p'), runs: [{ text: 'Area ' }, inlineMath('x^2'), { text: ' is small.' }] }] });
    const source = createJinshuSource({ key: 'r', initialDoc: doc });
    const res = source.apply({ batchId: 'b', origin: AGENT, base: source.getState().revision, mode: 'suggest', ops: [{ op: 'edit', id: 'p', find: 'small', replace: '<b>large</b>' }] });
    const sug = source.getState().suggestions.find(s => s.id === res.suggestionId);
    assert.equal(sug.kind, 'range');
    assert.deepEqual([sug.from, sug.to], [10, 15]);
    source.acceptSuggestion(res.suggestionId);
    const runs = source.getState().doc.blocks[0].runs;
    assert.equal(runs[1].math, 'x^2');
    assert.ok(runs.some(run => run.text === 'large' && run.bold), JSON.stringify(runs));
  });

  it('saves that overlap run one after another, each against the version the last returned', async () => {
    const bases = [];
    let open = 0;
    let most = 0;
    const source = createJinshuSource({
      key: 's',
      initialDoc: lesson(),
      save: async (_doc, base) => {
        open += 1;
        most = Math.max(most, open);
        bases.push(base);
        await wait(5);
        open -= 1;
        return { ok: true, version: `v${bases.length}` };
      },
    });
    source.apply({ batchId: 'a', origin: HUMAN, base: source.getState().revision, ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'One' }] });
    const first = source.save();
    await wait(1);
    source.apply({ batchId: 'b', origin: HUMAN, base: source.getState().revision, ops: [{ op: 'edit', id: 'p1', find: 'One', replace: 'Two' }] });
    await Promise.all([first, source.save()]);
    assert.equal(most, 1);
    assert.deepEqual(bases.slice(1), ['v1']);
  });

  it('forAgent({ since }) shows a top-level block when something inside it changed', () => {
    const source = createJinshuSource({ key: 'f', initialDoc: lesson() });
    const since = source.getState().revision;
    source.apply({ batchId: 'n', origin: HUMAN, base: since, ops: [{ op: 'edit', id: 'q1', find: 'Inner', replace: 'Nested' }] });
    assert.match(source.forAgent({ since }).view, /Nested/);
  });

  it('accepting an insert marks the new block, clears redo, and goes through validateBatch', () => {
    const seen = [];
    const source = createJinshuSource({ key: 'a', initialDoc: lesson(), validateBatch: (_next, _prev, ctx) => (seen.push(ctx.origin.kind), []) });
    source.apply({ batchId: 'e', origin: HUMAN, base: source.getState().revision, ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'One' }] });
    source.undo(undefined, HUMAN);
    assert.equal(source.getState().canRedo, true);
    const res = source.apply({ batchId: 'i', origin: AGENT, base: source.getState().revision, mode: 'suggest', ops: [{ op: 'insert', at: { after: 'p1' }, blocks: [paragraph('Added.', 'p9')] }] });
    seen.length = 0;
    source.acceptSuggestion(res.suggestionId);
    assert.deepEqual(seen, ['human']);
    assert.equal(source.getState().canRedo, false);
    assert.equal(source.forAgent({ since: source.getState().revision - 1 }).view.includes('Added.'), true);
    assert.equal(source.forAgent({ since: source.getState().revision - 1 }).view.includes('First.'), false, 'the anchor is not what changed');
  });

  it('an upload that returns extra fields still replaces the temporary path', async () => {
    const source = createJinshuSource({
      key: 'm',
      initialDoc: lesson(),
      media: {
        pick: async () => ({ file: { path: 'lx://temp/a.png', name: 'a.png', size: 10, type: 'image/png' } }),
        upload: async () => ({ src: 'https://cdn.example/a.png', etag: 'abc', url: 'https://cdn.example/a.png' }),
      },
    });
    const sent = [];
    const handlers = new Set();
    source.serve({ send: msg => sent.push(msg), on: (event, fn) => (event === 'data' && handlers.add(fn), () => handlers.delete(fn)) }, { role: 'editor' });
    [...handlers].forEach(fn => fn({ t: 'media', action: 'pick', kind: 'image', at: { after: 'p1' } }));
    await wait(10);
    const image = source.getState().doc.blocks.find(block => block.type === 'image');
    assert.equal(image.src, 'https://cdn.example/a.png');
  });

  it('a failed open is tried again on the next open', async () => {
    let loads = 0;
    const sources = createJinshuSources(key => ({ load: async () => { loads += 1; if (loads === 1) throw Error('offline'); return { doc: lesson(), version: 'v1' }; } }));
    await assert.rejects(sources.open('k'));
    assert.ok(await sources.open('k'));
    assert.equal(loads, 2);
  });

  it('a combined signal stops listening to a long-lived one once nothing waits on it', () => {
    const listeners = new Set();
    const long = { aborted: false, addEventListener: (_t, fn) => listeners.add(fn), removeEventListener: (_t, fn) => listeners.delete(fn) };
    for (let i = 0; i < 3; i += 1) {
      const both = anySignal(long);
      const done = () => {};
      both.addEventListener('abort', done);
      both.removeEventListener('abort', done);
    }
    assert.equal(listeners.size, 0);
  });

  it('a second stream is refused through its problems, not thrown', () => {
    const source = createJinshuSource({ key: 't', initialDoc: lesson() });
    source.stream({ at: { end: 'doc' }, origin: AGENT });
    const second = source.stream({ at: { end: 'doc' }, origin: AGENT });
    assert.equal(second.push('<p>x</p>').problems[0].code, 'REJECTED');
    assert.equal(second.finish().problems[0].code, 'REJECTED');
    assert.equal(plainText(source.getState().doc.blocks[1].runs), 'First.');
  });
});
