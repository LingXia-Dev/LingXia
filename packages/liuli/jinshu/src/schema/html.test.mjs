import { test } from 'node:test';
import assert from 'node:assert/strict';

import { checkDocument, createDocument, htmlToBlocks, plainText } from './index.ts';

const shape = blocks => JSON.parse(JSON.stringify(blocks, (key, value) => (key === 'id' ? undefined : value)));

test('htmlToBlocks: a web page keeps headings, paragraphs, lists, quotes, code, tables and marks', () => {
  const html = `<!DOCTYPE html><html><head><title>Page</title><style>b { color: red }</style></head><body>
<h1>Title &amp; more</h1>
<p>Some <strong>bold</strong>, <em>italic</em>, <code>code</code> and a <a href="https://e.com/?a=1&amp;b=2">link</a>.</p>
<ul><li>one<li>two<ul><li>nested</ul></ul>
<ol start=3><li><p>three</p><p>more</p></ol>
<blockquote><h2>Said</h2>quoted</blockquote>
<pre class="language-js">let a = 1;
  b &lt; c</pre>
<table><thead><tr><th>A<th>B</thead><tr><td>1<td>2<td>3</table>
<hr></body></html>`;
  const blocks = htmlToBlocks(html);
  assert.deepEqual(blocks.map(b => b.type), ['heading', 'paragraph', 'list', 'list', 'quote', 'code', 'table', 'divider']);
  assert.deepEqual(shape(blocks.slice(0, 2)), [
    { type: 'heading', level: 1, runs: [{ text: 'Title & more' }] },
    {
      type: 'paragraph',
      runs: [
        { text: 'Some ' }, { text: 'bold', bold: true }, { text: ', ' }, { text: 'italic', italic: true }, { text: ', ' },
        { text: 'code', code: true }, { text: ' and a ' }, { text: 'link', link: 'https://e.com/?a=1&b=2' }, { text: '.' },
      ],
    },
  ]);
  assert.deepEqual(shape(blocks[2].items), [
    { runs: [{ text: 'one' }] },
    { runs: [{ text: 'two' }], blocks: [{ type: 'list', style: 'bullet', items: [{ runs: [{ text: 'nested' }] }] }] },
  ]);
  assert.equal(blocks[3].style, 'ordered');
  assert.equal(blocks[3].start, 3);
  assert.deepEqual(shape(blocks[4].blocks), [
    { type: 'paragraph', runs: [{ text: 'Said', bold: true }] },
    { type: 'paragraph', runs: [{ text: 'quoted' }] },
  ], 'a quote holds no heading');
  assert.deepEqual(shape(blocks[5]), { type: 'code', language: 'js', code: 'let a = 1;\n  b < c' });
  assert.equal(blocks[6].headerRow, true);
  assert.deepEqual(blocks[6].rows.map(r => r.cells.length), [3, 3], 'a short row is padded');
  assert.deepEqual(checkDocument(createDocument({ blocks })).filter(p => p.severity === 'error'), []);
});

test('htmlToBlocks: untrusted content is dropped or kept as text', () => {
  const html =
    '<p onclick="steal()">Hi <a href="javascript:alert(1)">there</a> <a href="/relative">rel</a> <a href="#top">top</a>' +
    '<script>alert(1)</script><style>p{}</style><img src="javascript:x" alt="pic"><x-app-ref id="a1">atom</x-app-ref>' +
    '<iframe src="https://evil.example"><p>frame</p></iframe><svg><text>vector</text></svg><blink>old</blink></p>';
  const blocks = htmlToBlocks(html);
  assert.deepEqual(shape(blocks), [{ type: 'paragraph', runs: [{ text: 'Hi there rel topatomold' }] }]);
});

test('htmlToBlocks: word processor markup reads as marks; their wrapper and list bullets do not', () => {
  const docs = '<meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-1"><p><span style="font-weight:700">Bold</span><span style="font-style:italic"> it</span><span style="text-decoration:underline"> under</span><span style="vertical-align:super">2</span> plain</p></b>';
  assert.deepEqual(shape(htmlToBlocks(docs)), [
    { type: 'paragraph', runs: [{ text: 'Bold', bold: true }, { text: ' it', italic: true }, { text: ' under', underline: true }, { text: '2', script: 'super' }, { text: ' plain' }] },
  ]);
  const word = `<p class=MsoListParagraph><![if !supportLists]><span style='mso-list:Ignore'>·<span>&nbsp;&nbsp;</span></span><![endif]>Item<o:p></o:p></p>`;
  assert.deepEqual(shape(htmlToBlocks(word)), [{ type: 'paragraph', runs: [{ text: 'Item' }] }]);
});

test('htmlToBlocks: a task list, breaks, and whitespace collapse as a browser shows them', () => {
  const blocks = htmlToBlocks('<ul><li><input type="checkbox" checked> done</li><li><input type="checkbox"> todo</li></ul><p>  a\n   b<br>\n c  </p>');
  assert.deepEqual(shape(blocks), [
    { type: 'list', style: 'task', items: [{ checked: true, runs: [{ text: 'done' }] }, { runs: [{ text: 'todo' }] }] },
    { type: 'paragraph', runs: [{ text: 'a b' }, { break: true }, { text: 'c' }] },
  ]);
});

test('htmlToBlocks: deep nesting and lists past the limit stay valid', () => {
  const deep = htmlToBlocks('<span><b>'.repeat(5000) + 'deep');
  assert.equal(plainText(deep[0].runs), 'deep');
  const nested = htmlToBlocks('<ul><li>'.repeat(10) + 'x');
  assert.deepEqual(checkDocument(createDocument({ blocks: nested })).filter(p => p.severity === 'error'), []);
});

test('htmlToBlocks: a Markdown document keeps only what Markdown holds', () => {
  const blocks = htmlToBlocks('<p><u>u</u> <mark>m</mark> <b>b</b></p><h5>h</h5>', { format: 'markdown' });
  assert.deepEqual(shape(blocks), [
    { type: 'paragraph', runs: [{ text: 'u m ' }, { text: 'b', bold: true }] },
    { type: 'heading', level: 4, runs: [{ text: 'h' }] },
  ]);
});
