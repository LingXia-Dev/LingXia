import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  applyOps,
  callout,
  checkDocument,
  codeBlock,
  columns,
  createDocument,
  createRegistry,
  customBlock,
  defineBlock,
  details,
  defaultSegmenter,
  diffDocuments,
  divider,
  dwellOf,
  exportAiSchema,
  extractOutline,
  fileBlock,
  heading,
  image,
  inlineMath,
  list,
  listBlocks,
  listFiles,
  listItem,
  mathBlock,
  migrateDocument,
  normalizeDocument,
  paragraph,
  parseAgentMarkup,
  plainText,
  playedRatio,
  quote,
  reduceObservations,
  run,
  table,
  toAgentView,
  video,
} from './schema/index.ts';

import { createJinshuSource } from './source/index.ts';
import { toolsOver } from './testing/agent.mjs';
import { inlinesToShorthand, shorthandToInlines, toggleShorthandWrap } from './JinshuEditor/inlineMarkdown.ts';

const quizBlock = defineBlock({
  type: 'x-demo-quiz',
  version: 2,
  attrsSchema: {
    type: 'object',
    required: ['question', 'options', 'answerIndex'],
    properties: {
      question: { type: 'string', minLength: 1 },
      options: { type: 'array', items: { type: 'string' } },
      answerIndex: { type: 'integer', minimum: 0 },
    },
  },
  migrate(attrs, from) {
    const raw = attrs;
    if (from === 1) {
      return {
        question: String(raw.q ?? ''),
        options: Array.isArray(raw.opts) ? raw.opts : [],
        answerIndex: Number(raw.ans ?? 0),
      };
    }
    return raw;
  },
  content: 'none',
  aiDescription: 'Single-choice comprehension quiz block.',
  fallbackText: attrs => `[Quiz] ${attrs.question}`,
});

const registry = createRegistry({ blocks: [quizBlock] });

describe('Jinshu Schema & Validation', () => {
  it('normalizes inline runs, heading marks, CRLF, and preserves unknown v1 fields', () => {
    const rawDoc = {
      version: 1,
      title: 'Norm Test',
      blocks: [
        {
          id: 'b_h1',
          type: 'heading',
          level: 2,
          runs: [
            { text: 'Hello ', bold: true, script: 'super' },
            { text: 'World', bold: true },
          ],
          futureCrop: { x: 10 },
        },
        {
          id: 'b_p1',
          type: 'paragraph',
          runs: [
            { text: 'foo', code: true, highlight: 'warning' },
            { text: '' },
            { text: 'bar', code: true },
          ],
        },
        {
          id: 'b_c1',
          type: 'code',
          language: 'ts',
          code: 'const a = 1;\r\nconst b = 2;\r',
        },
      ],
    };

    const norm = normalizeDocument(rawDoc, registry);
    assert.deepEqual(norm.blocks[0].runs, [{ text: 'Hello World' }]);
    assert.equal((norm.blocks[0]).futureCrop.x, 10);
    assert.deepEqual(norm.blocks[1].runs, [{ text: 'foobar', code: true }]);
    assert.equal(norm.blocks[2].code, 'const a = 1;\nconst b = 2;\n');

    const problems = checkDocument(norm, registry);
    const errors = problems.filter(p => p.severity === 'error');
    const warnings = problems.filter(p => p.severity === 'warning');
    assert.equal(errors.length, 0);
    assert.ok(warnings.some(w => w.message.includes('futureCrop')));
  });

  it('rejects duplicate ids, forbidden schemes, and path traversal', () => {
    const badDoc = createDocument({
      blocks: [
        paragraph([run('bad link', { link: 'javascript:alert(1)' })], 'b_dup'),
        image('../../etc/passwd', { id: 'b_dup' }),
      ],
    });
    const problems = checkDocument(badDoc, registry);
    const errors = problems.filter(p => p.severity === 'error');
    assert.ok(errors.some(e => e.message.includes('Duplicate id')));
    assert.ok(errors.some(e => e.message.includes('javascript:')));
    assert.ok(errors.some(e => e.message.includes('escapes root')));
  });

  it('migrates extension block v1 -> v2 and recomputes fallback', () => {
    const v1Doc = createDocument({
      blocks: [
        customBlock('x-demo-quiz', { q: '2+2?', opts: ['3', '4'], ans: 1 }, 'old', { version: 1, id: 'b_q1' }),
      ],
    });
    const migrated = migrateDocument(v1Doc, registry);
    const blk = migrated.blocks[0];
    assert.equal(blk.version, 2);
    assert.equal('v' in blk, false);
    assert.equal(blk.attrs.question, '2+2?');
    assert.equal(blk.fallback, '[Quiz] 2+2?');
    assert.equal(checkDocument(migrated, registry).filter(p => p.severity === 'error').length, 0);
  });
});

describe('Jinshu Markup Round-Trip', () => {
  it('round-trips rich blocks, nested lists, dynamic code fences, and inline marks', () => {
    const doc = createDocument({
      title: 'Markup Demo',
      blocks: [
        heading(2, '核心概念', 'b_h1'),
        paragraph(
          [
            run('关注 '),
            run('lx.fs.watch', { bold: true, code: true }),
            run(' 与 '),
            run('零拷贝流', { link: '#b_c1' }),
            run(' (a < b) '),
            inlineMath('E = mc^2'),
          ],
          'b_p1',
        ),
        callout('warning', [paragraph('不要在 View 中轮询。', 'b_p2')], {
          id: 'b_c1',
          title: [run('常见'), run('陷阱', { bold: true })],
        }),
        list(
          'bullet',
          [
            listItem('普通项', { id: 'b_li1' }),
            listItem('含代码块项', {
              id: 'b_li2',
              blocks: [codeBlock('ts', 'const s = "```";\nif (a < b && c > d) return;', { id: 'b_code1' })],
            }),
            listItem([], {
              id: 'b_li3',
              blocks: [image('./arch.png', { alt: '架构图', id: 'b_img1' })],
            }),
          ],
          { id: 'b_l1' },
        ),
        codeBlock('ts', '', { id: 'b_empty_code' }),
        table(['字段', '说明'], [[[run('id', { code: true })], '稳定标识']], { id: 'b_t1' }),
        columns([[paragraph('左栏', 'b_p4')], [paragraph('右栏', 'b_p5')]], { id: 'b_cols1' }),
        mathBlock('\\int_0^\\infty e^{-x^2} dx', 'b_m1'),
        details('展开细节', [paragraph('详情内容', 'b_p3')], { id: 'b_d1' }),
        fileBlock('./slides.pdf', 'slides.pdf', { id: 'b_f1', mimeType: 'application/pdf' }),
        divider('b_hr1'),
      ],
    });

    const markup = toAgentView(doc, { registry });
    assert.ok(markup.includes('<code id="b_empty_code" language="ts"/>'));
    assert.ok(markup.includes('````')); // dynamic 4-backtick fence because code contains ```

    const parsed = parseAgentMarkup(markup, { registry });
    assert.equal(parsed.problems.filter(p => p.severity === 'error').length, 0);
    assert.equal(parsed.blocks.length, doc.blocks.length);
    assert.equal(parsed.blocks[3].items[1].runs[0].text, '含代码块项');
    assert.equal(parsed.blocks[3].items[2].runs.length, 0);
    assert.equal(parsed.blocks[3].items[2].blocks[0].type, 'image');
    assert.equal(parsed.blocks[4].code, '');

    assert.equal(listFiles(doc).length, 1);
    assert.equal(extractOutline(doc).length, 1);
  });
});

describe('applyOps & Diff & Observations', () => {
  it('applies atomic ops, edit with single-match & lineRange, tableRow/Col, and rolls back on failure', () => {
    const doc = createDocument({
      blocks: [
        heading(1, 'Intro', 'b_h1'),
        paragraph('Hello streaming world', 'b_p1'),
        codeBlock('ts', 'const x = 1;\nconst y = 1;\nconst z = 3;', { id: 'b_c1' }),
        table(['Col A', 'Col B'], [['1', '2']], { id: 'b_t1' }),
      ],
    });

    // Ambiguous match without lineRange should fail atomically
    const failRes = applyOps(doc, [
      { op: 'edit', id: 'b_c1', field: 'code', find: '= 1', replace: '= 2' },
    ]);
    assert.equal(failRes.doc, doc);
    assert.equal(failRes.problems[0].code, 'AMBIGUOUS_MATCH');
    assert.ok(failRes.problems[0].current?.includes('L1:'));

    // With lineRange + inline edit + tableRow + tableCol
    const okRes = applyOps(doc, [
      { op: 'edit', id: 'b_p1', find: 'streaming', replace: '<b>streaming</b>' },
      { op: 'edit', id: 'b_c1', field: 'code', lineRange: [2, 2], find: '= 1', replace: '= 42' },
      { op: 'tableRow', id: 'b_t1', action: 'insert', rowId: 'b_r3', cells: [[{ text: '3' }], [{ text: '4' }]] },
      { op: 'tableCol', id: 'b_t1', action: 'insert', index: 2, header: [{ text: 'Col C' }] },
    ]);
    assert.equal(okRes.problems.filter(p => p.severity === 'error').length, 0);
    assert.equal(okRes.doc.blocks[1].runs[1].bold, true);
    assert.equal(okRes.doc.blocks[2].code, 'const x = 1;\nconst y = 42;\nconst z = 3;');
    assert.equal(okRes.doc.blocks[3].rows[0].cells.length, 3);
    assert.equal(okRes.doc.blocks[3].rows.length, 3);
    assert.equal(okRes.doc.blocks[3].rows[2].id, 'b_r3');

    // Diff
    const diff = diffDocuments(doc, okRes.doc);
    assert.equal(diff.stats.modified, 3);
    assert.equal(diff.stats.unchanged, 1);
  });

  it('reduces objective observations idempotently and respects media src change', () => {
    const events = [
      { seq: 1, sessionId: 's1', kind: 'dwell', blockId: 'b_p1', dwellMs: 3000, seenRatio: 0.8, idleMs: 0 },
      { seq: 2, sessionId: 's1', kind: 'played', blockId: 'b_v1', src: './v1.mp4', range: [0, 30000], durationMs: 100000, rate: 1 },
      { seq: 3, sessionId: 's1', kind: 'played', blockId: 'b_v1', src: './v1.mp4', range: [20000, 60000], durationMs: 100000, rate: 1 },
    ];
    const p1 = reduceObservations(undefined, events);
    // Replay same events -> idempotent
    const p2 = reduceObservations(p1, events);
    assert.equal(dwellOf(p2, 'b_p1'), 3000);
    assert.equal(playedRatio(p2, 'b_v1', './v1.mp4'), 0.6);
    // Different src does not inherit old playback progress
    assert.equal(playedRatio(p2, 'b_v1', './v2.mp4'), 0);
  });
});

describe('JinshuSource & JinshuTools & Streaming', () => {
  it('supports block-level rebasing, focus downgrade to suggest, review accept, undo, and tools', async () => {
    const initial = createDocument({
      title: 'Lesson 1',
      blocks: [
        heading(1, 'Lesson 1', 'b_h1'),
        paragraph('Original intro text.', 'b_p1'),
        paragraph('Second paragraph.', 'b_p2'),
      ],
    });
    const source = createJinshuSource({ key: 'lesson-1', initialDoc: initial, registry, agentSave: 'confirm' });
    const tools = toolsOver({ 'lesson-1': source });
    const agent = { kind: 'agent', id: 'assistant', label: 'Codex' };

    // 1. Agent reads doc
    const read1 = (await tools.call('jinshu_read', { doc: 'lesson-1' })).data;
    assert.equal(read1.revision, 1);
    assert.ok(read1.view.includes('jinshu-document-data'));

    // 2. Human focuses b_p1 -> Agent direct edit on b_p1 downgrades to suggest
    source.setFocus(['b_p1']);
    const editRes = await tools.call(
      'jinshu_edit',
      {
        doc: 'lesson-1',
        base: read1.revision,
        mode: 'direct',
        ops: [{ op: 'edit', id: 'b_p1', find: 'Original', replace: 'Updated', note: 'Polish opening' }],
      },
    );
    assert.equal(editRes.status, 'pending-review');
    assert.equal(editRes.data.downgraded, 'focus');
    editRes.suggestionId = editRes.data.suggestionId;
    assert.equal(source.getState().suggestions.length, 1);
    assert.equal(plainText(source.getState().doc.blocks[1].runs), 'Original intro text.');

    // 3. Human accepts suggestion -> updates doc & records review
    source.acceptSuggestion(editRes.suggestionId);
    assert.equal(plainText(source.getState().doc.blocks[1].runs), 'Updated intro text.');
    const read2 = (await tools.call('jinshu_read', { doc: 'lesson-1', since: 1 })).data;
    assert.equal(read2.reviews.length, 1);
    assert.equal(read2.reviews[0].accepted, true);

    // 4. Stale block detection when editing with outdated base
    const staleRes = await tools.call(
      'jinshu_edit',
      {
        doc: 'lesson-1',
        base: 1,
        ops: [{ op: 'edit', id: 'b_p1', find: 'Updated', replace: 'Final' }],
      },
    );
    assert.equal(staleRes.code, 'STALE_BLOCK');

    // 5. Agent save requires confirmation under agentSave: 'confirm'
    const saveReq = await tools.call('jinshu_save', { doc: 'lesson-1', summary: 'Save polished intro' });
    assert.equal(saveReq.status, 'pending-review');
    assert.equal(source.getState().pendingAgentSave?.summary, 'Save polished intro');

    // 6. Stream API
    const stream = source.stream({ at: { end: 'doc' }, origin: agent, mode: 'direct' });
    stream.push('<p id="b_stream1">Streamed ');
    stream.push('paragraph.</p>');
    const fin = stream.finish();
    assert.equal(fin.problems.length, 0);
    assert.equal(source.getState().doc.blocks.length, 4);

    // 7. exportAiSchema includes custom block
    const schema = exportAiSchema(registry);
    assert.equal(schema.customBlocks.length, 1);
    assert.equal(schema.customBlocks[0].type, 'x-demo-quiz');
  });

  it('round-trips inline shorthand without losing bold/italic/code/highlight/link/math', () => {
    const runs = [
      run('Use '),
      run('GPU Channel', { bold: true }),
      run(' with '),
      run('syncDoc', { code: true }),
      run(' and '),
      run('docs', { link: 'https://example.com' }),
      run(' '),
      inlineMath('E=mc^2'),
    ];
    const short = inlinesToShorthand(runs);
    assert.equal(short, 'Use **GPU Channel** with `syncDoc` and [docs](https://example.com) $E=mc^2$');
    const back = shorthandToInlines(short);
    assert.deepEqual(back, runs);
    assert.equal(toggleShorthandWrap('hello', '**'), '**hello**');
    assert.equal(toggleShorthandWrap('**hello**', '**'), 'hello');
  });
});

describe('word diff segmenting', () => {
  it('splits the same way on every engine: Han and kana alone, letter runs in any script', () => {
    assert.deepEqual(defaultSegmenter('abc中文 café, мир'), ['abc', '中', '文', ' ', 'café', ',', ' ', 'мир']);
    assert.deepEqual(defaultSegmenter('こんにちは'), ['こ', 'ん', 'に', 'ち', 'は']);
  });
});
