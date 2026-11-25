import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  applyOps,
  callout,
  checkDocument,
  codeBlock,
  createDocument,
  details,
  divider,
  documentToMarkdown,
  formatAllows,
  heading,
  image,
  inlineMath,
  inlinesToShorthand,
  list,
  listItem,
  markdownToBlocks,
  markdownToDocument,
  mathBlock,
  normalizeDocument,
  paragraph,
  quote,
  run,
  shorthandToInlines,
  table,
  tableRow,
  video,
} from './index.ts';
import { trimLineEdges } from './inlineMarkdown.ts';

const formatErrors = problems => problems.filter(p => p.severity === 'error' && p.code === 'FORMAT');
const read = (text, previous) => markdownToDocument(text, previous ? { previous } : undefined);
const write = doc => documentToMarkdown(doc).text;
const md = lines => `${lines.join('\n')}\n`;

const isEmptyParagraph = b => b.type === 'paragraph' && b.runs.length === 0;

/** What Markdown can say of a document: no empty paragraphs, no spaces at line edges, an empty alt as none. */
function markdownForm(doc) {
  const runs = r => trimLineEdges(r);
  const blocks = (list, keepOne) => {
    const mapped = list.map(block);
    const out = mapped.filter(b => !isEmptyParagraph(b));
    return out.length || !keepOne ? out : [mapped.find(isEmptyParagraph) ?? { id: 'p-tmp', type: 'paragraph', runs: [] }];
  };
  const item = it => {
    let next = { ...it, runs: runs(it.runs) };
    const children = it.blocks ? blocks(it.blocks, false) : [];
    // An empty item's first paragraph is read as the item's text.
    if (next.runs.length === 0 && next.checked === undefined && children[0]?.type === 'paragraph') next.runs = children.shift().runs;
    delete next.blocks;
    if (children.length) next = { ...next, blocks: children };
    return next;
  };
  const block = b => {
    switch (b.type) {
      case 'paragraph':
      case 'heading':
        return { ...b, runs: runs(b.runs) };
      case 'quote':
      case 'callout':
        return { ...b, blocks: blocks(b.blocks, true) };
      case 'list':
        return { ...b, items: b.items.map(item) };
      case 'table':
        return { ...b, rows: b.rows.map(r => ({ ...r, cells: r.cells.map(runs) })) };
      case 'image': {
        if (b.alt !== '') return b;
        const { alt: _alt, ...rest } = b;
        return rest;
      }
      default:
        return b;
    }
  };
  const d = normalizeDocument(doc);
  const title = d.title?.trim();
  const { title: _t, ...rest } = d;
  return normalizeDocument({ ...rest, ...(title ? { title } : {}), blocks: blocks(d.blocks, true) });
}

/** doc → text → doc keeps the document as Markdown can say it, ids included, and the text is stable. */
function roundTrip(doc) {
  const want = markdownForm(doc);
  assert.deepEqual(formatErrors(checkDocument(want, undefined, { format: 'markdown' })), [], 'the document is in the Markdown profile');
  const out = documentToMarkdown(want);
  assert.deepEqual(out.problems, [], `writes exactly: ${JSON.stringify(out.text)}`);
  const back = read(out.text, want);
  assert.deepEqual(back.doc, want, `via ${JSON.stringify(out.text)}`);
  assert.equal(write(back.doc), out.text, 'text → doc → text is byte-identical');
  return out.text;
}

describe('Markdown blocks, both directions', () => {
  it('writes every allowed block in its plain syntax and reads it back', () => {
    const doc = createDocument({
      title: 'Week 3',
      blocks: [
        heading(2, 'Limits', 'h1'),
        paragraph([run('A '), run('limit', { bold: true }), run(' is '), run('where', { italic: true }), run(' '), inlineMath('f(x)'), run(' goes.')], 'p1'),
        quote([paragraph('Quoted', 'p2')], 'q1'),
        callout('warning', [paragraph('Careful', 'p3')], { id: 'c1' }),
        list('bullet', [listItem('one', { id: 'i1', blocks: [list('bullet', [listItem('deep', { id: 'i2' })], { id: 'l2' })] }), listItem('two', { id: 'i3' })], { id: 'l1' }),
        list('ordered', [listItem('first', { id: 'i4' }), listItem('second', { id: 'i5' })], { start: 3, id: 'l3' }),
        list('task', [listItem('todo', { id: 'i6', checked: false }), listItem('done', { id: 'i7', checked: true })], { id: 'l4' }),
        codeBlock('js', 'let a = 1;\n', { id: 'k1' }),
        mathBlock('\\int_0^1 x\\,dx', 'm1'),
        table(['x', 'f(x)'], [tableRow(['1', '2'], 'r2')], { id: 't1', headerRowId: 'r1', align: ['start', 'end'] }),
        divider('d1'),
        image('img/graph.png', { alt: 'A graph', id: 'g1' }),
      ],
    });
    const text = roundTrip(doc);
    assert.equal(
      text,
      md([
        '# Week 3',
        '',
        '## Limits',
        '',
        'A **limit** is *where* $f(x)$ goes.',
        '',
        '> Quoted',
        '',
        '> [!WARNING]',
        '> Careful',
        '',
        '- one',
        '  - deep',
        '- two',
        '',
        '3. first',
        '4. second',
        '',
        '- [ ] todo',
        '- [x] done',
        '',
        '```js',
        'let a = 1;',
        '',
        '```',
        '',
        '$$',
        '\\int_0^1 x\\,dx',
        '$$',
        '',
        '| x | f(x) |',
        '| :-- | --: |',
        '| 1 | 2 |',
        '',
        '---',
        '',
        '![A graph](img/graph.png)',
      ]),
    );
  });

  it('reads setext headings, nested quotes, ~~~ and indented code, ```math, autolinks and references', () => {
    const { doc, problems } = read(
      md([
        'Title',
        '=====',
        '',
        'Sub',
        '---',
        '',
        '> outer',
        '> > inner',
        '',
        '~~~py',
        'x = 1',
        '~~~',
        '',
        '    indented',
        '',
        '```math',
        'a^2',
        '```',
        '',
        'See <https://a.example>, www.b.example and [ref][r].',
        '',
        '[r]: https://r.example "Ref"',
      ]),
    );
    assert.equal(doc.title, 'Title');
    assert.equal(doc.blocks[0].type, 'heading');
    assert.equal(doc.blocks[0].level, 2);
    assert.equal(doc.blocks[1].blocks[1].type, 'quote');
    assert.deepEqual([doc.blocks[2].language, doc.blocks[2].code], ['py', 'x = 1']);
    assert.deepEqual([doc.blocks[3].language, doc.blocks[3].code], ['text', 'indented']);
    assert.deepEqual([doc.blocks[4].type, doc.blocks[4].latex], ['math', 'a^2']);
    const links = doc.blocks[5].runs.filter(r => r.link).map(r => [r.text, r.link]);
    assert.deepEqual(links, [['https://a.example', 'https://a.example'], ['www.b.example', 'http://www.b.example'], ['ref', 'https://r.example']]);
    assert.ok(problems.some(p => p.severity === 'warning' && /title "Ref" is dropped/.test(p.message)));
  });

  it('keeps lists apart, numbers from start, and keeps empty items', () => {
    roundTrip(createDocument({
      blocks: [
        list('bullet', [listItem('a', { id: 'a' })], { id: 'l1' }),
        list('bullet', [listItem('b', { id: 'b' })], { id: 'l2' }),
        list('ordered', [listItem('c', { id: 'c' })], { start: 0, id: 'l3' }),
        list('ordered', [listItem('d', { id: 'd' })], { start: 7, id: 'l4' }),
        paragraph([], 'e'),
        list('task', [listItem([], { id: 'f', checked: false }), listItem([], { id: 'g', blocks: [paragraph('x', 'h')] , checked: true})], { id: 'l5' }),
        paragraph([run('line'), { break: true }, { break: true }, run('after')], 'i'),
      ],
    }));
  });

  it('writes an empty document as an empty file and reads one back', () => {
    const empty = createDocument({ blocks: [paragraph([], 'only')] });
    assert.equal(write(empty), '');
    assert.deepEqual(read('', empty).doc, normalizeDocument(empty));
  });
});

describe('Markdown marks, both directions', () => {
  it('writes bold, italic, strike, code, links, math and breaks', () => {
    const runs = [
      run('b', { bold: true }), run(' '), run('i', { italic: true }), run(' '), run('s', { strike: true }), run(' '),
      run('c', { code: true }), run(' '), run('l', { link: 'https://x.example' }), run(' '), inlineMath('x'),
      { break: true }, run('bi', { bold: true, italic: true }), run('b', { bold: true }),
    ];
    const text = roundTrip(createDocument({ blocks: [paragraph(runs, 'p')] }));
    assert.equal(text, md(['**b** *i* ~~s~~ `c` [l](https://x.example) $x$\\', '***bi*b**']));
  });

  it('writes a plain URL bare and reads bare URLs as links', () => {
    const text = roundTrip(createDocument({ blocks: [paragraph([run('see '), run('https://x.example/a', { link: 'https://x.example/a' }), run(' now')], 'p')] }));
    assert.equal(text, md(['see https://x.example/a now']));
  });

  it('keeps Chinese text with marks, quotes beside a mark included', () => {
    assert.equal(roundTrip(createDocument({ blocks: [paragraph([run('加粗', { bold: true }), run('文字')], 'p')] })), md(['**加粗**文字']));
    const text = roundTrip(createDocument({ blocks: [paragraph([run('“加粗”', { bold: true }), run('文字')], 'p')] }));
    assert.match(text, /^\*\*“加粗”\*\*&#25991;字\n$/, 'an entity keeps the closing ** a closer');
    assert.deepEqual(read('**加粗**文字\n').doc.blocks[0].runs, [{ text: '加粗', bold: true }, { text: '文字' }]);
  });
});

describe('Markdown escaping is minimal and exact', () => {
  const textOf = s => write(createDocument({ blocks: [paragraph(s, 'p')] }));
  const cases = [
    ['3*4*5', '3\\*4\\*5'],
    ['a_b_c', 'a_b_c'],
    ['#hashtag', '#hashtag'],
    ['# not a heading', '\\# not a heading'],
    ['1. not a list', '1\\. not a list'],
    ['- not a list', '\\- not a list'],
    ['> not a quote', '\\> not a quote'],
    ['costs $5', 'costs $5'],
    ['$5 and $6', '\\$5 and \\$6'],
    ['a `tick`', 'a \\`tick\\`'],
    ['[2](附录)', '\\[2](附录)'],
    ['C++ and a+b=c', 'C++ and a+b=c'],
    ['www.example.com', 'www\\.example.com'],
    ['<b>', '\\<b>'],
    ['&amp;', '\\&amp;'],
    [' padded ', 'padded'],
    ['[x] box', '\\[x] box'],
    ['---', '\\---'],
    ['50% done.', '50% done.'],
  ];
  for (const [text, want] of cases) {
    it(`${JSON.stringify(text)} → ${JSON.stringify(want)}`, () => {
      assert.equal(textOf(text), `${want}\n`);
      roundTrip(createDocument({ blocks: [paragraph(text, 'p')] }));
    });
  }

  it('escapes pipes in table cells, code spans included, and keeps CJK cells with marks', () => {
    const doc = createDocument({
      blocks: [
        table(
          [[run('名称')], [run('值', { bold: true })]],
          [tableRow([[run('a|b')], [run('x|y', { code: true })]], 'r2'), tableRow([[run('中文', { italic: true }), run('说明')], []], 'r3')],
          { id: 't', headerRowId: 'r1' },
        ),
      ],
    });
    const text = roundTrip(doc);
    assert.equal(text, md(['| 名称 | **值** |', '| --- | --- |', '| a\\|b | `x\\|y` |', '| *中文*说明 |  |']));
  });

  it('chooses a code span fence longer than the backticks inside', () => {
    const text = roundTrip(createDocument({ blocks: [paragraph([run('a ` b', { code: true }), run(' and '), run('`x`', { code: true })], 'p')] }));
    assert.equal(text, md(['``a ` b`` and `` `x` ``']));
  });

  it('chooses a fence longer than the backticks in a code block', () => {
    const text = roundTrip(createDocument({ blocks: [codeBlock('md', '```js\nx\n```', { id: 'k' })] }));
    assert.equal(text, md(['````md', '```js', 'x', '```', '````']));
  });

  it('reads CRLF line endings', () => {
    const { doc } = read('# T\r\n\r\nline one\\\r\nline two\r\n\r\n- a\r\n- b\r\n');
    assert.equal(doc.title, 'T');
    assert.deepEqual(doc.blocks[0].runs, [{ text: 'line one' }, { break: true }, { text: 'line two' }]);
    assert.equal(doc.blocks[1].items.length, 2);
  });

  it('joins soft-wrapped lines with a space, and Chinese lines without one', () => {
    const { doc } = read('one\ntwo\n\n中文\n文字\n');
    assert.deepEqual(doc.blocks[0].runs, [{ text: 'one two' }]);
    assert.deepEqual(doc.blocks[1].runs, [{ text: '中文文字' }]);
  });
});

describe('Markdown round trip holds for any document in the profile', () => {
  it('300 random documents: doc → text → doc keeps everything, text → doc → text is identical', () => {
    let seed = 20261125;
    const rand = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 0x100000000);
    const pick = arr => arr[Math.floor(rand() * arr.length)];
    const words = ['a', 'b c', '*', '加粗', '“引号”', '2*3', '_x_', '#', '1.', '- ', '[x]', '$', '|', '`', '<a>', '&amp;', '\\', ' ', 'www.a.b', 'https://e.x/y', '😀', '~', '> q'];
    let n = 0;
    const id = p => `${p}${n++}`;
    const runs = ({ breaks = true, cell = false } = {}) => {
      const out = [];
      for (let k = Math.floor(rand() * 5); k > 0; k--) {
        if (breaks && rand() < 0.08) { out.push({ break: true }); continue; }
        if (rand() < 0.08) { out.push(inlineMath(pick(['x', 'a^2', '\\frac{1}{2}']))); continue; }
        const marks = {};
        if (rand() < 0.25) marks.bold = true;
        if (rand() < 0.25) marks.italic = true;
        if (rand() < 0.15) marks.strike = true;
        if (rand() < 0.12) marks.code = true;
        if (rand() < 0.12) marks.link = pick(['https://x.example/a', 'https://x.example/(b)', '#h0']);
        const word = pick(words);
        // A cell cannot hold code with a backslash before a pipe.
        out.push(run(cell && word === '\\' ? '/' : word, Object.keys(marks).length ? marks : undefined));
      }
      return out;
    };
    const itemChild = depth => {
      const r = rand();
      if (r < 0.4) return paragraph(runs(), id('p'));
      if (r < 0.6 && depth < 5) return randomList(depth + 1);
      if (r < 0.8) return codeBlock(pick(['text', 'js']), pick(['a', '', 'x\n\ny', '```']), { id: id('k') });
      return image(pick(['a.png', 'https://i.example/b c.png']), { alt: pick([undefined, 'alt', '[x]*y*']), id: id('g') });
    };
    const randomList = depth => {
      const style = pick(['bullet', 'ordered', 'task']);
      const items = [];
      for (let k = 1 + Math.floor(rand() * 3); k > 0; k--) {
        const blocks = rand() < 0.3 ? [itemChild(depth)] : undefined;
        items.push(listItem(runs(), { id: id('i'), ...(style === 'task' ? { checked: rand() < 0.5 } : {}), ...(blocks ? { blocks } : {}) }));
      }
      return list(style, items, { id: id('l'), ...(style === 'ordered' && rand() < 0.5 ? { start: Math.floor(rand() * 20) } : {}) });
    };
    const block = (depth, inQuote) => {
      const r = rand();
      if (r < 0.25) return paragraph(runs(), id('p'));
      if (r < 0.35 && !inQuote) return heading(1 + Math.floor(rand() * 4), runs({ breaks: false }), id('h'));
      if (r < 0.42 && depth < 2) return quote([block(depth + 1, true), ...(rand() < 0.5 ? [block(depth + 1, true)] : [])], id('q'));
      if (r < 0.48 && depth < 2) return callout(pick(['info', 'tip', 'warning', 'danger']), [block(depth + 1, false)], { id: id('c') });
      if (r < 0.62) return randomList(1);
      if (r < 0.7) return codeBlock(pick(['text', 'js', 'c++']), pick(['x', 'a\n\n  b', '```\ny', '', 'tab\tx']), { id: id('k') });
      if (r < 0.76) return mathBlock(pick(['x', 'a\\\\\nb', '$$', '']), id('m'));
      if (r < 0.86) {
        const cols = 1 + Math.floor(rand() * 3);
        const cells = () => Array.from({ length: cols }, () => runs({ breaks: false, cell: true }));
        return table(cells(), [tableRow(cells(), id('r'))], { id: id('t'), headerRowId: id('r'), ...(rand() < 0.5 ? { align: Array.from({ length: cols }, () => pick(['start', 'center', 'end'])) } : {}) });
      }
      if (r < 0.92) return divider(id('d'));
      return image('img/x.png', { alt: pick([undefined, 'a picture']), id: id('g') });
    };
    for (let doc = 0; doc < 300; doc++) {
      const blocks = Array.from({ length: 1 + Math.floor(rand() * 6) }, () => block(0, false));
      const title = rand() < 0.3 ? pick(['Title', '# Hash', 'a *b*']) : undefined;
      const meta = rand() < 0.15 ? { frontMatter: pick(['', 'tags: [a]', 'title: x\nn: 1']) } : undefined;
      roundTrip(createDocument({ blocks, ...(title ? { title } : {}), ...(meta ? { meta } : {}) }));
    }
  });
});

describe('Markdown ids', () => {
  const base = createDocument({
    title: 'T',
    blocks: [paragraph('first', 'p-a'), paragraph('second', 'p-b'), list('bullet', [listItem('x', { id: 'li-x' }), listItem('y', { id: 'li-y' })], { id: 'l-1' }), paragraph('third', 'p-c')],
  });
  const text = write(base);

  it('derives the same valid, unique ids from the same text', () => {
    const a = read(text).doc;
    const b = read(text).doc;
    assert.deepEqual(a, b);
    const ids = [...a.blocks.map(x => x.id), ...a.blocks[2].items.map(i => i.id)];
    assert.ok(ids.every(x => /^[a-z0-9_-]{1,32}$/.test(x)));
    assert.equal(new Set(ids).size, ids.length);
    const twice = read('same\n\nsame\n').doc;
    assert.notEqual(twice.blocks[0].id, twice.blocks[1].id);
  });

  it('keeps previous ids for unchanged blocks, an edited block in place and moved neighbours', () => {
    const edited = text.replace('second', 'second, edited elsewhere').replace('first', 'new paragraph\n\nfirst');
    const { doc } = read(edited, normalizeDocument(base));
    assert.deepEqual(doc.blocks.map(b => b.id).slice(1), ['p-a', 'p-b', 'l-1', 'p-c']);
    assert.notEqual(doc.blocks[0].id, 'p-a');
    assert.deepEqual(doc.blocks[3].items.map(i => i.id), ['li-x', 'li-y']);
    assert.equal(doc.blocks[2].runs[0].text, 'second, edited elsewhere');
  });

  it('reads a leading # line as the title, or as a heading when the previous document had one there', () => {
    const withHeading = normalizeDocument(createDocument({ blocks: [heading(1, 'Intro', 'h-1'), paragraph('x', 'p-1')] }));
    const out = write(withHeading);
    assert.equal(out, md(['# Intro', '', 'x']));
    assert.equal(read(out).doc.title, 'Intro');
    assert.deepEqual(read(out, withHeading).doc, withHeading);
    assert.equal(read('# **Bold** title\n').doc.title, undefined, 'a formatted heading stays a heading');
  });

  it('keeps front matter verbatim', () => {
    const src = md(['---', 'tags: [a, b]', 'draft: true', '---', '', '# Note', '', 'body']);
    const { doc } = read(src);
    assert.deepEqual(doc.meta, { frontMatter: 'tags: [a, b]\ndraft: true' });
    assert.equal(write(doc), src);
  });
});

describe('Markdown says what it could not keep', () => {
  const warnings = text => read(text).problems.filter(p => p.severity === 'warning' && p.code === 'FORMAT').map(p => p.message);

  it('warns when reading what Markdown-in-Jinshu cannot hold', () => {
    assert.ok(warnings('<div>x</div>\n').some(m => /Raw HTML is kept as text/.test(m)));
    assert.ok(warnings('a <span>b</span>\n').some(m => /Raw HTML/.test(m)));
    assert.ok(warnings('###### six\n').some(m => /level-6 heading is read as level 4/.test(m)));
    assert.ok(warnings('> # h\n').some(m => /heading inside a quote/.test(m)));
    assert.ok(warnings('see ![i](a.png) here\n').some(m => /image inside text/.test(m)));
    assert.ok(warnings('> [!IMPORTANT]\n> x\n').some(m => /no callout tone/.test(m)));
    assert.ok(warnings('> [!NOTE] Title\n> x\n').some(m => /callout title/.test(m)));
    assert.ok(warnings('[2](附录)\n').some(m => /kept as text/.test(m)));
    assert.equal(read('[2](附录)\n').doc.blocks[0].runs[0].text, '[2](附录)', 'an invalid link keeps every character');
  });

  it('checkDocument refuses what the profile does not allow, and applyOps with it', () => {
    const doc = createDocument({
      lang: 'en',
      blocks: [
        paragraph([run('u', { underline: true }), run('h', { highlight: 'warning' })], 'p'),
        details('More', [paragraph('x', 'p2')], { id: 'd' }),
        image('a.png', { alt: '', caption: 'c', id: 'g' }),
        codeBlock('js', 'x', { filename: 'a.js', id: 'k' }),
        table(null, [['a']], { id: 't' }),
        callout('tip', [paragraph('y', 'p3')], { title: 'Title', id: 'c' }),
      ],
    });
    const errors = formatErrors(checkDocument(doc, undefined, { format: 'markdown' }));
    const text = errors.map(e => `${e.id ?? '-'} ${e.message}`).join('\n');
    for (const want of ['- Markdown cannot hold the document\'s "lang".', 'p Markdown cannot hold underline.', 'p Markdown cannot hold highlight.', 'd Markdown cannot hold "details" blocks.', 'g Markdown cannot hold "caption"', 'k Markdown cannot hold "filename"', 't A Markdown table needs a header row', 'c Markdown cannot hold "title"']) {
      assert.ok(text.includes(want), `missing: ${want}\n${text}`);
    }
    assert.ok(!/alt/.test(text), 'an empty alt is the same as none');
    assert.deepEqual(formatErrors(checkDocument(doc)), [], 'JSON holds all of it');
    const ok = createDocument({ blocks: [paragraph('x', 'p')] });
    const res = applyOps(ok, [{ op: 'replace', id: 'p', blocks: [paragraph([run('x', { underline: true })], 'p')] }], undefined, { format: 'markdown' });
    assert.ok(res.problems.some(p => p.code === 'FORMAT' && p.severity === 'error'));
    assert.equal(res.doc, ok, 'the edit is refused');
  });

  it('documentToMarkdown degrades what it cannot hold and lists each loss', () => {
    const doc = createDocument({
      blocks: [
        details('Summary', [paragraph('body', 'p1')], { id: 'd' }),
        video('https://v.example/a.mp4', { id: 'v' }),
        paragraph([run('under', { underline: true }), run(' x')], 'p2'),
      ],
    });
    const out = documentToMarkdown(doc);
    assert.equal(out.text, md(['**Summary**', '', 'body', '', '[https://v.example/a.mp4](https://v.example/a.mp4)', '', 'under x']));
    assert.deepEqual(out.problems.map(p => p.id).sort(), ['d', 'p2', 'v']);
    assert.ok(out.problems.every(p => p.severity === 'error' && p.code === 'FORMAT'));
  });
});

describe('formatAllows', () => {
  it('lets JSON hold everything and Markdown its profile', () => {
    assert.equal(formatAllows('json', { block: 'video' }), true);
    assert.equal(formatAllows('markdown', { block: 'video' }), false);
    assert.equal(formatAllows('markdown', { block: 'callout' }), true);
    assert.equal(formatAllows('markdown', { block: 'callout', field: 'title' }), false);
    assert.equal(formatAllows('markdown', { block: 'image', field: 'alt' }), true);
    assert.equal(formatAllows('markdown', { block: 'image', field: 'caption' }), false);
    assert.equal(formatAllows('markdown', { mark: 'strike' }), true);
    assert.equal(formatAllows('markdown', { mark: 'underline' }), false);
    assert.equal(formatAllows('markdown', { mark: 'atom' }), false);
    assert.equal(formatAllows('markdown', { mark: 'math' }), true);
  });
});

describe('Pasting Markdown', () => {
  it('gives fresh ids and keeps the Jinshu shorthand in JSON documents', () => {
    const blocks = markdownToBlocks('# H\n\n==mark== and ++u++\n\n- a\n- b\n');
    assert.deepEqual(blocks.map(b => b.type), ['heading', 'paragraph', 'list']);
    assert.ok(blocks[1].runs.some(r => r.highlight === 'accent'));
    assert.ok(blocks[1].runs.some(r => r.underline));
    const plain = markdownToBlocks('==mark==\n', { format: 'markdown' });
    assert.deepEqual(plain[0].runs, [{ text: '==mark==' }]);
    assert.notEqual(markdownToBlocks('x\n')[0].id, markdownToBlocks('x\n')[0].id);
  });

  it('shorthand in the markdown dialect writes only what Markdown holds', () => {
    const runs = [run('a', { bold: true }), run('b', { underline: true })];
    assert.equal(inlinesToShorthand(runs, { dialect: 'markdown' }), '**a**b');
    assert.deepEqual(shorthandToInlines('++b++ www.x.com', { dialect: 'markdown' }), [{ text: '++b++ ' }, { text: 'www.x.com', link: 'http://www.x.com' }]);
  });
});

describe('Markdown leaves what it does not need to say', () => {
  it('does not write empty paragraphs or spaces at line edges', () => {
    const doc = createDocument({ blocks: [paragraph([], 'a'), paragraph(' padded ', 'b'), paragraph([run('x '), { break: true }, run(' y')], 'c'), quote([paragraph([], 'd')], 'q')] });
    assert.equal(write(doc), md(['padded', '', 'x\\', 'y', '', '>']));
    assert.deepEqual(checkDocument(doc, undefined, { format: 'markdown' }).filter(p => p.code === 'FORMAT'), []);
    roundTrip(doc);
  });

  it('writes an empty alt as no alt, without complaint', () => {
    const doc = createDocument({ blocks: [image('a.png', { alt: '', id: 'g' })] });
    assert.deepEqual(documentToMarkdown(doc), { text: md(['![](a.png)']), problems: [] });
  });
});

describe('Markdown task lists', () => {
  it('is a task list only when every item has a box; otherwise the box is the item\'s text', () => {
    const src = md(['- 一', '- [x] 任务']);
    const { doc } = read(src);
    assert.equal(doc.blocks[0].style, 'bullet');
    assert.deepEqual(doc.blocks[0].items.map(i => i.runs), [[{ text: '一' }], [{ text: '[x] 任务' }]]);
    assert.equal(write(doc), src, 'written back without escaping and without new boxes');
    assert.equal(read(md(['- [ ] a', '- [x] b'])).doc.blocks[0].style, 'task');
  });

  it('keeps an empty task item as a bare box', () => {
    const text = roundTrip(createDocument({ blocks: [list('task', [listItem([], { id: 'a', checked: false }), listItem('b', { id: 'b', checked: true })], { id: 'l' })] }));
    assert.equal(text, md(['- [ ]', '- [x] b']));
  });
});

describe('Markdown keeps the bytes of what did not change', () => {
  const sample = [
    '---',
    'title: Notes',
    'tags: [a]',
    '---',
    '# Weekly notes',
    '',
    'This paragraph is hard',
    'wrapped at a narrow',
    'width, as people do.',
    '',
    '| Name | Value |',
    '|:---|---:|',
    '| a  |  1 |',
    '',
    '<div align="center">',
    '  raw html',
    '</div>',
    '',
    '- 一',
    '- [x] 任务',
    '',
    '*   odd   spacing item',
    '*   another',
    '',
    'Setext Heading',
    '--------------',
    '',
    '',
    '[ref]: https://example.com',
    'See [the ref][ref].   ',
    '',
  ].join('\n');

  it('writes an unedited file back byte for byte', () => {
    for (const text of [sample, sample.replace(/\n/g, '\r\n'), `﻿${sample}`, sample.trimEnd(), '', '\n\n', '[r]: https://r.example\n', '&nbsp;\n']) {
      const { doc, source } = read(text);
      assert.equal(documentToMarkdown(doc, { source }).text, text, JSON.stringify(text.slice(0, 30)));
    }
  });

  it('writes any file back byte for byte', () => {
    let seed = 7;
    const rand = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 0x100000000);
    const pick = arr => arr[Math.floor(rand() * arr.length)];
    const lines = ['', '', 'text', 'wrapped\ncontinued', '# H', 'Setext', '===', '---', '- a', '- [x] b', '* c', '1. one', '2) two', '   - indented', '> quote', '> [!NOTE]', '> > deep', '```js', '```', '~~~', '    code', '$$', 'x^2', '| a | b |', '|:---|---:|', '| 1 | 2 |', '<div>', '</div>', '[r]: https://r.example', 'see [r]', '![i](a.png)', '  edge  ', '\ttab', '***', 'a  ', 'b\\', '&nbsp;', '中文', '---\nfm: 1\n---'];
    for (let n = 0; n < 500; n++) {
      const text = Array.from({ length: Math.floor(rand() * 12) }, () => pick(lines)).join(pick(['\n', '\r\n'])) + pick(['', '\n', '\n\n']);
      const { doc, source } = read(text);
      assert.equal(documentToMarkdown(doc, { source }).text, text, JSON.stringify(text));
    }
  });

  it('rewrites only the lines of an edited block', () => {
    const { doc, source } = read(sample);
    const edited = structuredClone(doc);
    const i = edited.blocks.findIndex(b => b.type === 'paragraph');
    edited.blocks[i].runs = [{ text: 'Rewritten.' }];
    const out = documentToMarkdown(edited, { source }).text;
    assert.equal(out, sample.replace('This paragraph is hard\nwrapped at a narrow\nwidth, as people do.', 'Rewritten.'));
    const crlf = read(sample.replace(/\n/g, '\r\n'));
    const e2 = structuredClone(crlf.doc);
    e2.blocks[i].runs = [{ text: 'Rewritten.' }];
    assert.equal(documentToMarkdown(e2, { source: crlf.source }).text, out.replace(/\n/g, '\r\n'), 'keeps the file\'s line endings');
  });

  it('inserts, deletes and moves blocks without disturbing the rest', () => {
    const { doc, source } = read(sample);
    const d = structuredClone(doc);
    const [moved] = d.blocks.splice(0, 1);
    d.blocks.push(moved);
    d.blocks.splice(1, 0, paragraph('New', 'p-new'), list('bullet', [listItem('x', { id: 'i-new' })], { id: 'l-new' }));
    const out = documentToMarkdown(d, { source }).text;
    assert.ok(out.includes('<div align="center">\n  raw html\n</div>'), 'untouched blocks keep their bytes');
    const strip = x => JSON.stringify(x, (k, v) => (k === 'id' ? undefined : v));
    assert.equal(strip(markdownForm(read(out, { previous: d }).doc)), strip(markdownForm(d)));
  });

  it('without a source writes the canonical form', () => {
    const { doc } = read(sample);
    const out = write(doc);
    assert.ok(out.includes('| :-- | --: |') && out.includes('\\<div'));
  });
});

describe('Jinshu shorthand tones', () => {
  it('reads a space after =={tone} as content and writes it back the same', () => {
    const runs = shorthandToInlines('=={warning} warm!==');
    assert.deepEqual(runs, [{ text: ' warm!', highlight: 'warning' }]);
    assert.equal(inlinesToShorthand(runs), '=={warning} warm!==');
  });
});
