import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  applyOps,
  audio,
  callout,
  checkDocument,
  codeBlock,
  columns,
  createDocument,
  createRegistry,
  customBlock,
  defineBlock,
  defineInline,
  details,
  divider,
  exportAiSchema,
  fileBlock,
  heading,
  image,
  inlineAtom,
  inlineKind,
  inlineMath,
  list,
  listItem,
  mathBlock,
  normalizeDocument,
  paragraph,
  parseAgentMarkup,
  plainText,
  quote,
  run,
  table,
  tableRow,
  toAgentView,
  video,
} from './schema/index.ts';
import { createJinshuSource } from './source/index.ts';
import { toolsOver } from './testing/agent.mjs';
import { atomsIn, inlinesToShorthand, shorthandToInlines } from './JinshuEditor/inlineMarkdown.ts';

const quiz = defineBlock({
  type: 'x-demo-quiz',
  version: 2,
  attrsSchema: {
    type: 'object',
    required: ['question'],
    properties: { question: { type: 'string', minLength: 1 }, answerIndex: { type: 'integer', minimum: 0 } },
    additionalProperties: false,
  },
  content: 'none',
  aiDescription: 'Quiz.',
  fallbackText: attrs => `[Quiz] ${attrs.question}`,
});
const aside = defineBlock({
  type: 'x-demo-aside',
  version: 1,
  attrsSchema: { type: 'object' },
  content: { blocks: 'paragraph+' },
  aiDescription: 'Aside.',
  fallbackText: () => '[Aside]',
});
const term = defineInline({
  inline: 'x-demo-term',
  version: 1,
  attrsSchema: { type: 'object', properties: { key: { type: 'string' } } },
  aiDescription: 'Glossary term.',
  fallbackText: attrs => String(attrs.key),
});
const registry = createRegistry({ blocks: [quiz, aside], inlines: [term] });

const errors = problems => problems.filter(p => p.severity === 'error');

/** Markup → document the way an agent's full rewrite would read it. */
function docFromMarkup(markup, existingAtoms) {
  const parsed = parseAgentMarkup(markup, { registry, existingAtoms });
  assert.deepEqual(errors(parsed.problems), [], 'markup parses cleanly');
  return normalizeDocument(
    {
      version: parsed.version ?? 1,
      ...(parsed.id !== undefined ? { id: parsed.id } : {}),
      ...(parsed.title !== undefined ? { title: parsed.title } : {}),
      ...(parsed.lang !== undefined ? { lang: parsed.lang } : {}),
      ...(parsed.meta !== undefined ? { meta: parsed.meta } : {}),
      blocks: parsed.blocks,
    },
    registry,
  );
}

/** Every block kind, every field, every inline kind and mark. */
export function kitchenSinkDoc() {
  const term1 = inlineAtom('x-demo-term', { key: 'GPU' }, 'GPU', { id: 'at_1' });
  return createDocument({
    id: 'sink',
    title: 'Kitchen <sink> & "quotes"',
    lang: 'zh-CN',
    meta: { course: 42, tags: ['a', 'b'] },
    blocks: [
      heading(1, [run('Title '), run('it', { italic: true })], 'h1'),
      heading(2, 'Second', 'h2'),
      paragraph(
        [
          run('  lead & <tag> "q"  '),
          run('b', { bold: true }),
          run('i', { italic: true }),
          run('u', { underline: true }),
          run('s', { strike: true }),
          run('c*d', { code: true }),
          run('sub', { script: 'sub' }),
          run('sup', { script: 'super' }),
          run('acc', { highlight: 'accent' }),
          run('warn', { highlight: 'warning', bold: true }),
          run('ok', { highlight: 'success' }),
          run('bad', { highlight: 'danger' }),
          run('ext', { link: 'https://example.com/a?b=1&c="2"' }),
          run('in-page', { link: '#h2', bold: true, italic: true }),
          { break: true },
          inlineMath('a < b & c'),
          term1,
          run(' tail'),
        ],
        'p1',
      ),
      quote([paragraph('quoted', 'q1p')], 'q1'),
      callout('tip', [paragraph('body', 'c1p')], { id: 'c1', title: [run('Ti'), run('tle', { underline: true })] }),
      callout('danger', [paragraph('untitled', 'c2p')], { id: 'c2' }),
      details('Summary', [paragraph('hidden', 'd1p')], { id: 'd1', defaultOpen: true }),
      details('Closed', [paragraph('hidden2', 'd2p')], { id: 'd2' }),
      list('task', [
        listItem('done', { id: 'li1', checked: true }),
        listItem('todo', { id: 'li2' }),
      ], { id: 'l1' }),
      list('ordered', [
        listItem('three', { id: 'li3', blocks: [codeBlock('ts', 'x()', { id: 'lc1' }), list('bullet', [listItem('deep', { id: 'li4' })], { id: 'l3' })] }),
        listItem([], { id: 'li5', blocks: [image('./a.png', { alt: '', id: 'limg' })] }),
      ], { id: 'l2', start: 3 }),
      codeBlock('python', 'def f():\n    return "```"\n', { id: 'code1', filename: 'f.py', highlightLines: [1, 2] }),
      codeBlock('text', '', { id: 'code2' }),
      mathBlock('\\frac{a}{b} < 1 & x', 'm1'),
      table(['H1', [run('H2', { bold: true })]], [tableRow(['a', 'b'], 'r1'), tableRow(['', [run('x', { code: true })]], 'r2')], {
        id: 't1',
        align: ['start', 'center'],
        widths: [2, 1],
        headerRowId: 'r0',
      }),
      table(null, [tableRow(['only'], 'r9')], { id: 't2' }),
      columns([{ id: 'col1', blocks: [paragraph('L', 'cp1')] }, { id: 'col2', blocks: [paragraph('R', 'cp2')] }], { id: 'cols', widths: [1, 2] }),
      divider('hr1'),
      image('./img.png', { id: 'img1', alt: 'Alt "x"', width: 480, height: 270, size: 'medium', caption: [run('cap '), run('b', { bold: true })] }),
      video('https://cdn.example.com/v.mp4', {
        id: 'v1',
        poster: './poster.png',
        durationMs: 125400,
        caption: 'Video caption',
        chapters: [{ atMs: 0, title: 'Intro' }, { atMs: 3200, title: '第二 <章>' }],
        subtitles: [{ src: './zh.vtt', lang: 'zh-CN', label: '中文' }],
      }),
      audio('./a.mp3', {
        id: 'a1',
        title: 'Episode',
        cover: './cover.png',
        durationMs: 61001,
        chapters: [{ atMs: 500, title: 'Start' }],
        transcript: [paragraph('spoken', 'tp1')],
      }),
      fileBlock('./slides.pdf', 'slides.pdf', { id: 'f1', display: 'inline', mimeType: 'application/pdf', sizeBytes: 123456, pageCount: 12, description: 'Deck' }),
      customBlock('x-demo-quiz', { question: '1+1?', answerIndex: 1 }, '', { id: 'x1', version: 2 }),
      customBlock('x-demo-aside', { tone: 'calm' }, '[Aside]', { id: 'x2', blocks: [paragraph('aside body', 'x2p')] }),
      customBlock('x-other-note', { n: [1, { k: 'v' }] }, 'Note fallback', { id: 'x3', runs: [run('note '), run('text', { bold: true })] }),
    ],
  });
}

describe('Markup is lossless', () => {
  it('normalize(parse(toAgentView(doc))) equals normalize(doc) for a doc with every block and inline kind', () => {
    const doc = normalizeDocument(kitchenSinkDoc(), registry);
    assert.deepEqual(errors(checkDocument(doc, registry)), []);
    const markup = toAgentView(doc, { registry, maxChars: Infinity });
    assert.deepEqual(docFromMarkup(markup), doc);
    // Canonical spellings: same names and units as the JSON fields.
    for (const piece of ['durationMs="125400"', 'width="480"', 'widths="2,1"', 'align="start,center"', 'highlightLines="1,2"', 'language="python"', 'defaultOpen="true"', '<chapter atMs="3200">', '<track src="./zh.vtt" lang="zh-CN" label="中文"/>', '<tr id="r1">', 'headerRow="true"', '<attrs>', 'sizeBytes="123456"', 'pageCount="12"']) {
      assert.ok(markup.includes(piece), `markup has ${piece}`);
    }
  });

  it('keeps lang without a title and a non-default version in the <doc> wrapper', () => {
    const doc = normalizeDocument(createDocument({ lang: 'en', blocks: [paragraph('x', 'p')] }));
    assert.equal(docFromMarkup(toAgentView(doc)).lang, 'en');
  });

  it('rewrite with an unmodified toAgentView changes nothing', () => {
    const doc = normalizeDocument(kitchenSinkDoc(), registry);
    const res = applyOps(doc, [{ op: 'rewrite', markup: toAgentView(doc, { registry, maxChars: Infinity }) }], registry);
    assert.deepEqual(errors(res.problems), []);
    assert.deepEqual(res.doc, doc);
  });
});

describe('Forward compatibility', () => {
  it('preserves unknown keys at every level and never rewrites version', () => {
    const raw = {
      version: 1,
      futureRoot: 'r',
      blocks: [
        { id: 'p', type: 'paragraph', runs: [{ text: 'a', futureMark: 1 }], futureBlock: true },
        { id: 'l', type: 'list', style: 'bullet', items: [{ id: 'li', runs: [], futureItem: 2 }] },
        { id: 'c', type: 'columns', columns: [{ id: 'c1', blocks: [paragraph('x', 'cx1')], w: 1 }, { id: 'c2', blocks: [paragraph('y', 'cx2')] }] },
        { id: 't', type: 'table', rows: [{ id: 'r', cells: [[]], futureRow: 3 }] },
        { id: 'x', type: 'x-a-b', version: 1, attrs: {}, fallback: 'f', futureExt: 4 },
        { id: 'v', type: 'video', src: 'a.mp4', chapters: [{ atMs: 1, title: 't', futureChapter: 5 }] },
      ],
    };
    const n = normalizeDocument(raw);
    assert.equal(n.futureRoot, 'r');
    assert.equal(n.blocks[0].futureBlock, true);
    assert.equal(n.blocks[0].runs[0].futureMark, 1);
    assert.equal(n.blocks[1].items[0].futureItem, 2);
    assert.equal(n.blocks[2].columns[0].w, 1);
    assert.equal(n.blocks[3].rows[0].futureRow, 3);
    assert.equal(n.blocks[4].futureExt, 4);
    assert.equal(n.blocks[5].chapters[0].futureChapter, 5);
    assert.equal(normalizeDocument({ ...raw, version: 3 }).version, 3);
  });

  it('a newer document or extension block is read-only: applyOps rejects with VERSION, checkDocument only warns', () => {
    const newer = normalizeDocument({ version: 2, blocks: [paragraph('x', 'p')] });
    const res = applyOps(newer, [{ op: 'delete', id: 'p' }]);
    assert.equal(res.doc, newer);
    assert.equal(res.problems[0].code, 'VERSION');
    assert.deepEqual(errors(checkDocument(newer)), []);

    const extNewer = createDocument({ blocks: [customBlock('x-demo-quiz', { question: 'q' }, '', { id: 'x', version: 3 }), paragraph('p', 'p')] });
    const res2 = applyOps(extNewer, [{ op: 'delete', id: 'p' }], registry);
    assert.equal(res2.problems[0].code, 'VERSION');
    assert.ok(checkDocument(extNewer, registry).some(p => p.code === 'VERSION' && p.severity === 'warning'));
  });

  it('has no legacy spellings: old shapes are simply invalid, and an invalid stored draft is discarded', async () => {
    const old = { version: 1, blocks: [{ id: 't', type: 'table', header: [[{ text: 'A' }]], rows: [[[{ text: '1' }]]] }, { id: 'x', type: 'x-a-b', v: 2, attrs: {}, fallback: 'f' }] };
    const problems = checkDocument(normalizeDocument(old));
    assert.ok(errors(problems).some(p => p.message.includes('row')), 'old table rows are invalid');
    assert.ok(problems.some(p => p.message.includes('"header"')), 'old header key is unknown');
    assert.equal(normalizeDocument(old).blocks[1].v, 2, 'v is an unknown key, not version');
    const map = new Map([['jinshu:draft:k', { doc: old }]]);
    const storage = { get: async k => map.get(k), set: async (k, v) => map.set(k, v), remove: async k => map.delete(k) };
    const source = createJinshuSource({ key: 'k', storage, initialDoc: createDocument({ blocks: [paragraph('x', 'p')] }) });
    assert.equal(await source.restoreDraft(), false);
    assert.equal(map.has('jinshu:draft:k'), false);
  });
});

describe('Validation completeness', () => {
  const bad = (block, message) => {
    const problems = errors(checkDocument(createDocument({ blocks: [block] }), registry));
    assert.ok(problems.length > 0, `${message}: expected an error`);
  };

  it('checks enums and types from the field table', () => {
    bad({ ...image('a.png', { alt: '' }), id: 'i', size: 'huge' }, 'size');
    bad({ ...fileBlock('a.pdf', 'a'), id: 'f', display: 'popup' }, 'display');
    bad(paragraph([{ text: 'x', highlight: 'neon' }], 'p'), 'highlight');
    bad(paragraph([{ text: 'x', script: 'mid' }], 'p'), 'script');
    bad(paragraph([{ text: 'x', bold: 'yes' }], 'p'), 'bold');
    bad({ ...heading(1, 'x', 'h'), level: 9 }, 'level');
    bad({ ...image('a.png', { alt: '' }), id: 'i', width: -3 }, 'width');
  });

  it('checks every URL-like field', () => {
    bad(audio('a.mp3', { id: 'a', cover: 'javascript:alert(1)' }), 'audio.cover');
    bad(video('a.mp4', { id: 'v', poster: 'data:text/html,x' }), 'poster');
    bad(video('a.mp4', { id: 'v', subtitles: [{ src: '../../etc/x.vtt', lang: 'en', label: 'EN' }] }), 'subtitle src');
    bad(fileBlock('vbscript:x', 'a', { id: 'f' }), 'file src');
  });

  it('enforces the extension content model', () => {
    bad(customBlock('x-demo-quiz', { question: 'q' }, '', { id: 'x', version: 2, runs: [run('no')] }), 'content none');
    bad(customBlock('x-demo-aside', {}, '', { id: 'x', runs: [run('no')] }), 'block content with runs');
  });

  it('reports conflicting inline keys as an error and dangling #links as warnings', () => {
    bad(paragraph([{ text: 'x', math: 'y' }], 'p'), 'conflict');
    assert.equal(inlineKind({ text: 'x', math: 'y' }), undefined);
    const problems = checkDocument(createDocument({ blocks: [paragraph([run('go', { link: '#nowhere' })], 'p')] }));
    assert.ok(problems.some(p => p.severity === 'warning' && p.message.includes('#nowhere')));
    assert.deepEqual(errors(problems), []);
  });
});

describe('applyOps is constrained and never throws', () => {
  const base = () =>
    createDocument({
      blocks: [
        heading(1, 'T', 'h'),
        paragraph([run('Hello '), run('bold world', { bold: true }), run(' end')], 'p'),
        image('a.png', { id: 'img', alt: '' }),
        table(['A', 'B'], [tableRow(['1', '2'], 'r1'), tableRow(['3', '4'], 'r2')], { id: 't', headerRowId: 'r0' }),
        list('task', [listItem('a', { id: 'la' }), listItem('b', { id: 'lb' })], { id: 'l' }),
        customBlock('x-demo-quiz', { question: 'q' }, '', { id: 'x', version: 2 }),
      ],
    });

  it('validates update values before normalizing', () => {
    const doc = base();
    const r1 = applyOps(doc, [{ op: 'update', id: 'h', attrs: { level: 9 } }], registry);
    assert.equal(r1.doc, doc);
    assert.equal(r1.problems[0].code, 'INVALID');
    const r2 = applyOps(doc, [{ op: 'update', id: 'img', attrs: { caption: 'text' } }], registry);
    assert.equal(r2.problems[0].code, 'INVALID');
    const r3 = applyOps(doc, [{ op: 'update', id: 'img', attrs: { colour: 'red' } }], registry);
    assert.equal(r3.problems[0].code, 'UNKNOWN_FIELD');
    for (const key of ['runs', 'type', 'id', 'rows']) {
      const r = applyOps(doc, [{ op: 'update', id: key === 'rows' ? 't' : 'p', attrs: { [key]: [] } }], registry);
      assert.equal(r.problems[0].code, 'UNKNOWN_FIELD', key);
    }
    const r4 = applyOps(doc, [{ op: 'update', id: 'x', attrs: { bogus: 1 } }], registry);
    assert.equal(r4.problems[0].code, 'UNKNOWN_FIELD');
    const ok = applyOps(doc, [{ op: 'update', id: 'img', attrs: { size: 'small', caption: [run('c')] } }, { op: 'update', id: 'la', attrs: { checked: true } }], registry);
    assert.deepEqual(errors(ok.problems), []);
    assert.equal(ok.doc.blocks[2].size, 'small');
    assert.equal(ok.doc.blocks[4].items[0].checked, true);
    assert.equal('checked' in ok.doc.blocks[4].items[1], false, 'checked:false is omitted');
  });

  it('survives malformed ops without throwing', () => {
    const doc = base();
    const junk = [null, 1, 'x', {}, { op: 'nope' }, { op: 'insert' }, { op: 'insert', at: 5, markup: '<p>x</p>' }, { op: 'insert', at: { end: 'doc' }, blocks: [null] },
      { op: 'insert', at: { end: 'doc' }, blocks: [{ type: 'table', rows: 'x' }] }, { op: 'replace', id: 'p', markup: 42 }, { op: 'edit', id: 'p' },
      { op: 'edit', id: 'p', find: 'Hello', replace: null }, { op: 'update', id: 'p' }, { op: 'update', id: 'img', attrs: { caption: 'text' } },
      { op: 'update', id: 'img', attrs: { caption: [null, 5] } }, { op: 'tableRow', id: 't', action: 'insert', cells: 'x' }, { op: 'tableRow', id: 't', action: 'warp' },
      { op: 'tableCol', id: 't', action: 'insert', index: 'a' }, { op: 'move', id: 'p' }, { op: 'setMeta', title: 5 }, { op: 'rewrite' },
      { op: 'text', target: null }, { op: 'text', target: { blockId: 'p' }, from: 0, to: 1, insert: 'x' }, { op: 'mark', target: { blockId: 'p' }, from: 0, to: 2, add: { bold: 'yes' } },
      { op: 'split', id: 'p', at: 'x' }, { op: 'setType', id: 'p', type: 'weird' }, { op: 'update', id: 'h', attrs: { level: '2' } }];
    for (const op of junk) {
      let res;
      assert.doesNotThrow(() => { res = applyOps(doc, [op], registry); }, JSON.stringify(op));
      assert.equal(res.doc, doc, `rejected: ${JSON.stringify(op)}`);
      assert.ok(errors(res.problems).length > 0, `reported: ${JSON.stringify(op)}`);
    }
    assert.doesNotThrow(() => applyOps(doc, 'not an array', registry));
    assert.doesNotThrow(() => checkDocument({ version: 1, blocks: [null, 5, { type: 'list', items: [null] }, { id: 't', type: 'table', rows: [null, { id: 'r', cells: [null] }] }] }));
  });

  it('addresses table rows by id', () => {
    const doc = base();
    const ins = applyOps(doc, [{ op: 'tableRow', id: 't', action: 'insert', rowId: 'rn', after: 'r1', cells: [[run('n')]] }], registry);
    assert.deepEqual(errors(ins.problems), []);
    assert.deepEqual(ins.doc.blocks[3].rows.map(r => r.id), ['r0', 'r1', 'rn', 'r2']);
    const mv = applyOps(ins.doc, [{ op: 'tableRow', id: 't', action: 'move', rowId: 'r2', before: 'r1' }], registry);
    assert.deepEqual(mv.doc.blocks[3].rows.map(r => r.id), ['r0', 'r2', 'r1', 'rn']);
    const del = applyOps(mv.doc, [{ op: 'tableRow', id: 't', action: 'delete', rowId: 'r1' }], registry);
    assert.deepEqual(del.doc.blocks[3].rows.map(r => r.id), ['r0', 'r2', 'rn']);
    assert.equal(applyOps(doc, [{ op: 'tableRow', id: 't', action: 'delete', rowId: 'zz' }], registry).problems[0].code, 'NOT_FOUND');
    const cell = applyOps(doc, [{ op: 'edit', id: 't', field: { cell: ['r2', 1] }, find: '4', replace: '44' }], registry);
    assert.equal(plainText(cell.doc.blocks[3].rows[2].cells[1]), '44');
  });

  it('edit matches plain text across marks and keeps the marks at the match', () => {
    const doc = base();
    const across = applyOps(doc, [{ op: 'edit', id: 'p', find: 'Hello bold', replace: 'Hi' }], registry);
    assert.deepEqual(errors(across.problems), []);
    assert.deepEqual(across.doc.blocks[1].runs, [{ text: 'Hi' }, { text: ' world', bold: true }, { text: ' end' }]);
    const inherit = applyOps(doc, [{ op: 'edit', id: 'p', find: 'world', replace: 'planet' }], registry);
    assert.deepEqual(inherit.doc.blocks[1].runs, [{ text: 'Hello ' }, { text: 'bold planet', bold: true }, { text: ' end' }]);
    const marked = applyOps(doc, [{ op: 'edit', id: 'p', find: 'end', replace: '<i>fin</i>' }], registry);
    assert.deepEqual(marked.doc.blocks[1].runs.at(-1), { text: 'fin', italic: true });
    assert.equal(applyOps(doc, [{ op: 'edit', id: 'p', find: '<b>bold', replace: 'x' }], registry).problems[0].code, 'NO_MATCH');
  });

  it('setType only converts without loss', () => {
    const doc = createDocument({
      blocks: [
        list('bullet', [listItem('a', { id: 'a' }), listItem('b', { id: 'b' })], { id: 'l2' }),
        list('bullet', [listItem([run('one', { bold: true })], { id: 'c' })], { id: 'l1' }),
        codeBlock('ts', 'a\nb', { id: 'cm' }),
        codeBlock('ts', 'ab', { id: 'cs' }),
        image('a.png', { id: 'img', alt: '' }),
        paragraph([run('x', { bold: true })], 'pb'),
        paragraph('plain', 'pp'),
      ],
    });
    const lost = (op, word) => {
      const res = applyOps(doc, [op]);
      assert.equal(res.doc, doc);
      assert.equal(res.problems[0].code, 'INVALID');
      assert.match(res.problems[0].message, word);
    };
    lost({ op: 'setType', id: 'l2', type: 'paragraph' }, /list item/);
    lost({ op: 'setType', id: 'cm', type: 'paragraph' }, /line breaks/);
    lost({ op: 'setType', id: 'img', type: 'paragraph' }, /image/);
    lost({ op: 'setType', id: 'pb', type: 'heading', level: 2 }, /bold/);
    lost({ op: 'setType', id: 'pb', type: 'code' }, /formatting/);
    const ok = applyOps(doc, [
      { op: 'setType', id: 'l1', type: 'paragraph' },
      { op: 'setType', id: 'cs', type: 'paragraph' },
      { op: 'setType', id: 'pp', type: 'heading', level: 3 },
      { op: 'setType', id: 'l2', type: 'list', style: 'ordered' },
    ]);
    assert.deepEqual(errors(ok.problems), []);
    assert.deepEqual(ok.doc.blocks[1], { id: 'l1', type: 'paragraph', runs: [{ text: 'one', bold: true }] });
    assert.deepEqual(ok.doc.blocks[3].runs, [{ text: 'ab' }]);
    assert.equal(ok.doc.blocks[6].level, 3);
    assert.equal(ok.doc.blocks[0].style, 'ordered');
  });
});

describe('Inline atoms have identity', () => {
  it('gives atoms unique ids and keeps attrs when markup echoes one by id', () => {
    const atom = inlineAtom('x-demo-term', { key: 'GPU' }, 'GPU', { id: 'at_g' });
    const doc = normalizeDocument(createDocument({ blocks: [paragraph([run('see '), atom], 'p'), paragraph([{ ...atom }], 'q')] }), registry);
    assert.equal(doc.blocks[0].runs[1].id, 'at_g');
    assert.notEqual(doc.blocks[1].runs[0].id, 'at_g', 'a repeated atom id is replaced');
    const res = applyOps(doc, [{ op: 'replace', id: 'p', markup: '<p>now <x-demo-term id="at_g"/></p>' }], registry);
    assert.deepEqual(errors(res.problems), []);
    assert.deepEqual(res.doc.blocks[0].runs[1], { id: 'at_g', inline: 'x-demo-term', version: 1, attrs: { key: 'GPU' }, fallback: 'GPU' });
  });
});

describe('Editor shorthand round-trips exactly', () => {
  const roundTrip = (runs, options) => {
    const short = inlinesToShorthand(runs);
    assert.deepEqual(shorthandToInlines(short, { existingAtoms: atomsIn(runs), ...options }), runs, `via ${JSON.stringify(short)}`);
    return short;
  };

  it('keeps literal delimiters literal', () => {
    assert.equal(roundTrip([run('2*3*4')]), '2\\*3\\*4');
    roundTrip([run('a_b ~c~ ==d== ++e++ ^f^ `g` [h](i) $j$ \\k ⟦l⟧ {m}')]);
    roundTrip([run('{x}', { highlight: 'accent' })]);
  });

  it('keeps atoms, highlight tones and stacked marks', () => {
    roundTrip([
      run('a '),
      inlineAtom('x-demo-term', { key: 'k' }, 'Te#rm⟧', { id: 'at_1' }),
      run(' w', { highlight: 'warning' }),
      run('s', { highlight: 'success', bold: true }),
      run('bi', { bold: true, italic: true }),
      run('b', { bold: true }),
      run('st', { strike: true, script: 'sub' }),
      run('code`tick', { code: true, bold: true }),
      run('lnk', { link: 'https://e.com/(x)', underline: true }),
      { break: true },
      inlineMath('x^2_{i}'),
    ]);
  });

  it('round-trips random mark combinations', () => {
    let seed = 7;
    const rand = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 0x100000000);
    const pick = arr => arr[Math.floor(rand() * arr.length)];
    const words = ['a', 'b c', '*', '中文', '2*3', '~', '=', '[x]', '$'];
    for (let n = 0; n < 300; n++) {
      const runs = [];
      for (let k = 0; k < 1 + Math.floor(rand() * 5); k++) {
        const marks = {};
        if (rand() < 0.3) marks.bold = true;
        if (rand() < 0.3) marks.italic = true;
        if (rand() < 0.2) marks.underline = true;
        if (rand() < 0.2) marks.strike = true;
        if (rand() < 0.15) marks.code = true;
        else {
          if (rand() < 0.15) marks.script = pick(['sub', 'super']);
          if (rand() < 0.2) marks.highlight = pick(['accent', 'warning', 'success', 'danger']);
        }
        if (rand() < 0.15) marks.link = 'https://x.y/z';
        runs.push(run(pick(words), Object.keys(marks).length ? marks : undefined));
      }
      const doc = normalizeDocument(createDocument({ blocks: [paragraph(runs, 'p')] }));
      roundTrip(doc.blocks[0].runs);
    }
  });
});

describe('Agent tools', () => {
  it('jinshu_edit without base is a validation error, and operationsSchema is a per-op oneOf', async () => {
    const source = createJinshuSource({ key: 'd', initialDoc: createDocument({ blocks: [paragraph('x', 'p')] }) });
    const res = await toolsOver({ d: source }).call('jinshu_edit', { doc: 'd', ops: [{ op: 'delete', id: 'p' }] });
    assert.equal(res.code, 'INVALID');
    assert.equal(source.getState().revision, 1);
    const schema = exportAiSchema(registry).operationsSchema;
    assert.ok(Array.isArray(schema.items.oneOf));
    assert.ok(schema.items.oneOf.length >= 10);
  });
});
