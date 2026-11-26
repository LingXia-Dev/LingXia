import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  applyOps,
  callout,
  createDocument,
  createRegistry,
  heading,
  paragraph,
  parseAgentMarkup,
  plainText,
  table,
} from './schema/index';
import { createJinshuSource } from './source/index';
import { inlinesToShorthand, linkAt, markSpanAt, shorthandToInlines, splitAt, toggleMarkAt } from './JinshuEditor/inlineMarkdown';
import { tokenizeCodeLine } from './internal/codeTokens';

const registry = createRegistry([]);
const human = { kind: 'human', id: 'author', label: '作者' };
const agent = { kind: 'agent', id: 'assistant', label: 'AI 助手' };

/** Deterministic PRNG so fuzz failures reproduce. */
function rng(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 0x100000000;
  };
}

function fakeChannel() {
  const sent = [];
  const handlers = { data: [], close: [] };
  return {
    sent,
    channel: {
      send: msg => sent.push(msg),
      on(event, fn) {
        handlers[event].push(fn);
        return () => {};
      },
    },
    deliver: msg => handlers.data.forEach(fn => fn(msg)),
  };
}

function memoryStorage() {
  const map = new Map();
  return {
    map,
    async get(k) { return map.get(k); },
    async set(k, v) { map.set(k, JSON.parse(JSON.stringify(v))); },
    async remove(k) { map.delete(k); },
  };
}

const tick = () => new Promise(r => setTimeout(r, 0));

describe('UTF-16 surrogate safety', () => {
  const doc = createDocument({ blocks: [paragraph('笑😀脸', 'p1')] });

  it('rejects text, mark and split offsets inside an emoji', () => {
    // '笑' = 1 unit, '😀' = 2 units: offset 2 is inside the pair.
    for (const op of [
      { op: 'text', target: { blockId: 'p1' }, from: 2, to: 2, insert: [{ text: 'x' }] },
      { op: 'mark', target: { blockId: 'p1' }, from: 0, to: 2, add: { bold: true } },
      { op: 'split', id: 'p1', at: 2, newId: 'p2' },
    ]) {
      const res = applyOps(doc, [op], registry);
      assert.equal(res.doc, doc, `${op.op} must leave the document untouched`);
      assert.ok(res.problems.some(p => p.code === 'INVALID' && /surrogate/.test(p.message)), `${op.op} reports the split pair`);
    }
  });

  it('accepts offsets on pair boundaries', () => {
    const res = applyOps(doc, [{ op: 'split', id: 'p1', at: 3, newId: 'p2' }], registry);
    assert.deepEqual(res.problems.filter(p => p.severity === 'error'), []);
    assert.equal(plainText(res.doc.blocks[0].runs), '笑😀');
    assert.equal(plainText(res.doc.blocks[1].runs), '脸');
  });

  it('rejects negative and inverted ranges', () => {
    const res = applyOps(doc, [{ op: 'text', target: { blockId: 'p1' }, from: 3, to: 1, insert: [] }], registry);
    assert.ok(res.problems.some(p => p.code === 'INVALID'));
  });
});

describe('Format buttons act where the person is', () => {
  const at = (text, start, end = start) => ({ start, end });
  const show = r => `${r.text.slice(0, r.start)}[${r.text.slice(r.start, r.end)}]${r.text.slice(r.end)}`;

  it('wraps the selection, and the selection stays on the same words', () => {
    assert.equal(show(toggleMarkAt('Jinshu stores documents as JSON', at(0, 14, 23), '**', 'text')), 'Jinshu stores **[documents]** as JSON');
  });

  it('a selection ending in a space marks the word, not the space', () => {
    assert.equal(toggleMarkAt('read this now', at(0, 5, 10), '*', 'text').text, 'read *this* now');
  });

  it('inside a marked span, or selecting it with its marks, takes the mark off', () => {
    assert.equal(show(toggleMarkAt('先读题目，**再看解析**', at(0, 9), '**', '文本')), '先读题目，再看[]解析');
    assert.equal(toggleMarkAt('a **bold** b', at(0, 2, 10), '**', 'x').text, 'a bold b');
    assert.equal(toggleMarkAt('a **bold** b', at(0, 4, 8), '**', 'x').text, 'a bold b');
  });

  it('a caret in a word marks that word and stays put in it', () => {
    assert.equal(show(toggleMarkAt('stores documents as', at(0, 10), '`', 'code')), 'stores `doc[]uments` as');
  });

  it('a caret between words, or in CJK text, puts in a selected placeholder to type over', () => {
    assert.equal(show(toggleMarkAt('结论如下：', at(0, 5), '`', '文本')), '结论如下：`[文本]`');
    assert.equal(show(toggleMarkAt('先读题目再看', at(0, 2), '==', '文本')), '先读==[文本]==题目再看');
  });

  it('italic is not taken for half of bold, nor subscript for half of strike', () => {
    assert.equal(markSpanAt('a **b** c', at(0, 4), '*'), null);
    assert.deepEqual(markSpanAt('a **b** c', at(0, 4), '**'), { open: 2, close: 5 });
    assert.equal(markSpanAt('~~gone~~ H~2~O', at(0, 4), '~'), null);
    assert.deepEqual(markSpanAt('~~gone~~ H~2~O', at(0, 12), '~'), { open: 10, close: 12 });
  });

  it('bold and italic on the same words: each turns on and off by itself', () => {
    const both = toggleMarkAt('a **word** b', at(0, 4, 8), '*', 'x');
    assert.equal(both.text, 'a ***word*** b');
    assert.ok(markSpanAt(both.text, both, '**') && markSpanAt(both.text, both, '*'), 'both show as on');
    assert.equal(toggleMarkAt(both.text, both, '*', 'x').text, 'a **word** b');
    assert.equal(toggleMarkAt(both.text, both, '**', 'x').text, 'a *word* b');
  });

  it('a span on another line does not hold the selection', () => {
    assert.equal(markSpanAt('**a\nb**', at(0, 5), '**'), null);
  });

  it('a link shows the typed label, else the selected text, else its address, at the selection', () => {
    assert.equal(show(linkAt('see the docs here', at(0, 8, 12), 'https://x.dev', '')), 'see the [[docs]](https://x.dev) here');
    assert.equal(show(linkAt('see the docs here', at(0, 8, 12), 'https://x.dev', 'manual')), 'see the [[manual]](https://x.dev) here');
    assert.equal(show(linkAt('see ', at(0, 4), 'https://x.dev', '')), 'see [[https://x.dev]](https://x.dev)');
  });

  it('Enter in the middle of a mark keeps both halves formatted', () => {
    const marks = ['**', '*', '++', '~~', '`', '==', '^', '~'];
    assert.deepEqual(splitAt('Hello **world** now', at(0, 11), marks), ['Hello **wor**', '**ld** now']);
    assert.deepEqual(splitAt('a ***both*** b', at(0, 7), marks), ['a ***bo***', '***th*** b']);
    assert.deepEqual(splitAt('x **bold** y', at(0, 4), marks), ['x ', '**bold** y'], 'at the mark’s start the mark moves whole');
    assert.deepEqual(splitAt('x **bold** y', at(0, 8), marks), ['x **bold**', ' y'], 'at its end it stays whole');
    assert.deepEqual(splitAt('plain text', at(0, 5), marks), ['plain', ' text']);
  });

  it('round-trips underline, superscript and subscript', () => {
    const src = '水是 H~2~O，面积 m^2^，++重点++';
    const runs = shorthandToInlines(src);
    assert.ok(runs.some(r => r.script === 'sub' && r.text === '2'));
    assert.ok(runs.some(r => r.script === 'super' && r.text === '2'));
    assert.ok(runs.some(r => r.underline && r.text === '重点'));
    assert.equal(inlinesToShorthand(runs), src);
  });
});

describe('Fuzz: parsers never throw on malformed input', () => {
  const alphabet = ['<p>', '</p>', '<b>', '</b>', '<callout tone="x">', '</callout>', '<code', '"', "'", '`', '```', '$', '**', '*', '~~', '==', '[', '](', ')', 'javascript:alert(1)', '../', '\n', 'A', '中', '😀', '<', '>', '&', '&amp;', '<table>', '<tr>', '<td>', '<li>', '<ul>'];

  it('parseAgentMarkup and shorthandToInlines survive 1500 random strings', () => {
    const rand = rng(20261124);
    for (let i = 0; i < 1500; i++) {
      const len = 1 + Math.floor(rand() * 24);
      let s = '';
      for (let k = 0; k < len; k++) s += alphabet[Math.floor(rand() * alphabet.length)];
      assert.doesNotThrow(() => parseAgentMarkup(s, { registry }), `markup #${i}: ${JSON.stringify(s)}`);
      assert.doesNotThrow(() => shorthandToInlines(s), `shorthand #${i}: ${JSON.stringify(s)}`);
    }
  });

  it('never lets an unsafe link scheme through the markup parser', () => {
    for (const href of ['javascript:alert(1)', 'JavaScript:alert(1)', ' javascript:x', 'data:text/html,x', 'vbscript:x']) {
      const res = parseAgentMarkup(`<p><a href="${href}">x</a></p>`, { registry });
      const runs = res.blocks[0]?.runs ?? [];
      assert.ok(!runs.some(r => r.link), `blocked ${href}`);
      assert.equal(plainText(runs), 'x', 'link text is kept');
    }
  });

  it('keeps the text of an unsafe shorthand link so the edit still saves', () => {
    const runs = shorthandToInlines('see [docs](javascript:alert(1)) and [site](https://example.com)');
    assert.ok(!runs.some(r => r.link && /javascript/i.test(r.link)));
    assert.ok(runs.some(r => r.link === 'https://example.com'));
    const doc = createDocument({ blocks: [paragraph(runs, 'p')] });
    assert.deepEqual(applyOps(doc, [{ op: 'replace', id: 'p', blocks: [paragraph(runs, 'p')] }], registry).problems.filter(p => p.severity === 'error'), []);
  });
});

describe('Concurrent human + agent edits', () => {
  it('rebases agent edits on untouched blocks and refuses stale ones', () => {
    const source = createJinshuSource({
      key: 'c1',
      registry,
      initialDoc: createDocument({ blocks: [paragraph('Alpha', 'a'), paragraph('Beta', 'b')] }),
    });
    const base = source.getState().revision;
    // Human edits block a first.
    const h = source.apply({ batchId: 'h1', origin: human, base, ops: [{ op: 'edit', id: 'a', find: 'Alpha', replace: 'Alpha!' }] });
    assert.deepEqual(h.problems, []);
    // Agent still on the old base: block b is untouched, so it applies.
    const ok = source.apply({ batchId: 'g1', origin: agent, base, ops: [{ op: 'edit', id: 'b', find: 'Beta', replace: 'Beta?' }] });
    assert.deepEqual(ok.problems.filter(p => p.severity === 'error'), []);
    // Agent on the old base touching block a: stale.
    const stale = source.apply({ batchId: 'g2', origin: agent, base, ops: [{ op: 'edit', id: 'a', find: 'Alpha', replace: 'A' }] });
    assert.equal(stale.problems[0].code, 'STALE_BLOCK');
    const d = source.getState().doc;
    assert.equal(plainText(d.blocks[0].runs), 'Alpha!');
    assert.equal(plainText(d.blocks[1].runs), 'Beta?');
  });

  it('refuses to undo a human batch that the agent later overlapped', () => {
    const source = createJinshuSource({ key: 'c2', registry, initialDoc: createDocument({ blocks: [paragraph('One', 'a')] }) });
    const h = source.apply({ batchId: 'h1', origin: human, base: source.getState().revision, ops: [{ op: 'edit', id: 'a', find: 'One', replace: 'Two' }] });
    source.apply({ batchId: 'g1', origin: agent, base: h.revision, ops: [{ op: 'edit', id: 'a', find: 'Two', replace: 'Three' }] });
    const res = source.undo('h1', human);
    assert.equal(res.problems[0]?.code, 'REJECTED');
    assert.equal(plainText(source.getState().doc.blocks[0].runs), 'Three');
  });

  it('marks a pending agent suggestion stale when the human edits its block', () => {
    const source = createJinshuSource({ key: 'c3', registry, initialDoc: createDocument({ blocks: [paragraph('Draft', 'a')] }) });
    const r0 = source.getState().revision;
    const s = source.apply({ batchId: 'g1', origin: agent, base: r0, mode: 'suggest', ops: [{ op: 'replace', id: 'a', blocks: [paragraph('AI version', 'a')] }] });
    assert.ok(s.suggestionId);
    source.apply({ batchId: 'h1', origin: human, base: source.getState().revision, ops: [{ op: 'edit', id: 'a', find: 'Draft', replace: 'Mine' }] });
    const sug = source.getState().suggestions.find(x => x.id === s.suggestionId);
    assert.equal(sug?.status, 'stale');
    assert.equal(source.acceptSuggestion(s.suggestionId).problems[0].code, 'STALE_BLOCK');
  });
});

describe('Streaming', () => {
  it('previews the open trailing paragraph as it streams', () => {
    const source = createJinshuSource({ key: 's1', registry, initialDoc: createDocument({ blocks: [paragraph('x', 'p0')] }) });
    const st = source.stream({ at: { end: 'doc' }, origin: agent });
    const first = st.push('<p id="s1">hel');
    assert.equal(first.blocks.length, 1, 'half-written paragraph is already visible');
    assert.equal(plainText(first.blocks[0].runs), 'hel');
    const second = st.push('lo</p><p>wor');
    assert.equal(second.blocks.length, 2);
    st.push('ld</p>');
    const fin = st.finish();
    assert.deepEqual(fin.problems, []);
    assert.deepEqual(source.getState().doc.blocks.slice(1).map(b => plainText(b.runs)), ['hello', 'world']);
  });

  it('stops a live stream when the View sends streamAbort, keeping partial output as a suggestion', () => {
    const source = createJinshuSource({ key: 's2', registry, initialDoc: createDocument({ blocks: [paragraph('x', 'p0')] }) });
    const ch = fakeChannel();
    source.serve(ch.channel);
    const st = source.stream({ at: { end: 'doc' }, origin: agent });
    st.push('<p>partial answer</p><p>still typ');
    ch.deliver({ t: 'streamAbort', streamId: st.streamId });
    assert.ok(ch.sent.some(m => m.t === 'stream' && m.status === 'aborted'), 'View is told the stream stopped');
    assert.equal(source.getState().suggestions.length, 1, 'partial output kept for review');
    assert.equal(source.getState().doc.blocks.length, 1, 'nothing was applied directly');
    // A new stream may start after the abort.
    assert.doesNotThrow(() => source.stream({ at: { end: 'doc' }, origin: agent }).abort());
  });
});

describe('Crash-recovery draft', () => {
  it('persists unsaved edits and restores them into a fresh source', async () => {
    const storage = memoryStorage();
    const initialDoc = createDocument({ blocks: [paragraph('Saved text', 'a')] });
    const s1 = createJinshuSource({ key: 'd1', registry, initialDoc, storage });
    s1.apply({ batchId: 'h1', origin: human, base: s1.getState().revision, ops: [{ op: 'edit', id: 'a', find: 'Saved', replace: 'Unsaved' }] });
    await tick();
    assert.ok(storage.map.has('jinshu:draft:d1'), 'draft written to storage');

    const s2 = createJinshuSource({ key: 'd1', registry, initialDoc, storage });
    assert.equal(await s2.restoreDraft(), true);
    assert.equal(plainText(s2.getState().doc.blocks[0].runs), 'Unsaved text');

    await s2.save();
    await tick();
    assert.equal(storage.map.has('jinshu:draft:d1'), false, 'draft cleared once saved');
  });

  it('ignores a corrupt draft', async () => {
    const storage = memoryStorage();
    storage.map.set('jinshu:draft:d2', { doc: { nope: true } });
    const s = createJinshuSource({ key: 'd2', registry, initialDoc: createDocument({ blocks: [heading(1, 'T', 'h')] }), storage });
    assert.equal(await s.restoreDraft(), false);
    assert.equal(plainText(s.getState().doc.blocks[0].runs), 'T');
  });
});

describe('Code highlighting tokenizer', () => {
  it('colours keywords, strings, comments, numbers and types', () => {
    const kinds = tokenizeCodeLine('const n: Number = 42; // "x"', 'ts').map(t => t.kind);
    assert.ok(kinds.includes('keyword'));
    assert.ok(kinds.includes('type'));
    assert.ok(kinds.includes('number'));
    assert.equal(kinds[kinds.length - 1], 'comment');
    assert.ok(tokenizeCodeLine("print('hi')  # note", 'python').some(t => t.kind === 'string'));
    assert.ok(tokenizeCodeLine("print('hi')  # note", 'python').some(t => t.kind === 'comment'));
  });

  it('keeps plain text plain and caps the run count', () => {
    assert.deepEqual(tokenizeCodeLine('just words', 'text'), [{ kind: 'plain', text: 'just words' }]);
    const long = Array.from({ length: 200 }, (_, i) => `a${i} = 1;`).join(' ');
    const toks = tokenizeCodeLine(long, 'js', 56);
    assert.ok(toks.length <= 57, `tokens ${toks.length}`);
    assert.equal(toks.map(t => t.text).join(''), long, 'no text lost');
  });
});

describe('Ops for table and container editing', () => {
  it('moves table columns with alignment and rejects deleting the last one', () => {
    const doc = createDocument({ blocks: [table(['A', 'B'], [['1', '2']], { id: 't', align: ['start', 'end'] })] });
    const moved = applyOps(doc, [{ op: 'tableCol', id: 't', action: 'move', index: 0, toIndex: 1 }], registry);
    assert.deepEqual(moved.doc.blocks[0].rows[0].cells.map(plainText), ['B', 'A']);
    assert.deepEqual(moved.doc.blocks[0].align, ['end', 'start']);
    const one = applyOps(doc, [{ op: 'tableCol', id: 't', action: 'delete', index: 0 }], registry).doc;
    const last = applyOps(one, [{ op: 'tableCol', id: 't', action: 'delete', index: 0 }], registry);
    assert.ok(last.problems.some(p => p.code === 'INVALID'));
  });

  it('replaces a multi-paragraph callout body', () => {
    const doc = createDocument({ blocks: [callout('info', [paragraph('one', 'c1'), paragraph('two', 'c2')], { id: 'c' })] });
    const res = applyOps(doc, [{ op: 'replace', id: 'c', blocks: [{ ...doc.blocks[0], blocks: [paragraph('one!', 'c1'), paragraph('two', 'c2'), paragraph('three', 'c3')] }] }], registry);
    assert.deepEqual(res.problems.filter(p => p.severity === 'error'), []);
    assert.deepEqual(res.doc.blocks[0].blocks.map(b => plainText(b.runs)), ['one!', 'two', 'three']);
  });
});
