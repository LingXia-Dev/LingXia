import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { transformSync } from 'esbuild';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

const parser = (async () => {
  const dir = mkdtempSync(join(tmpdir(), 'liuli-markdown-'));
  writeFileSync(join(dir, 'parse.mjs'), transformSync(readFileSync(new URL('./parse.ts', import.meta.url), 'utf8'), { loader: 'ts' }).code);
  return import(join(dir, 'parse.mjs'));
})();

test('inline: marks nest, a link carries its runs, and what is not Markdown stays text', async () => {
  const { parseInline } = await parser;
  assert.deepEqual(parseInline('A **bold _both_** and `code`'), [
    { text: 'A ' }, { text: 'bold ', isStrong: true }, { text: 'both', isStrong: true, isEmphasis: true }, { text: ' and ' }, { text: 'code', isCode: true },
  ]);
  assert.deepEqual(parseInline('[a *link*](https://x.y) <https://a.b>'), [
    { text: 'a ', href: 'https://x.y' }, { text: 'link', href: 'https://x.y', isEmphasis: true }, { text: ' ' }, { text: 'https://a.b', href: 'https://a.b' },
  ]);
  assert.deepEqual(parseInline('snake_case_word 2 * 3 \\*not\\* ~~gone~~'), [{ text: 'snake_case_word 2 * 3 *not* ' }, { text: 'gone', isStruck: true }]);
  assert.deepEqual(parseInline('one\ntwo  \nthree'), [{ text: 'one two\nthree' }], 'a line end is a space; two spaces before it break the line');
  assert.deepEqual(parseInline('`a * b` and ![a photo](p.png)'), [{ text: 'a * b', isCode: true }, { text: ' and a photo' }], 'code is taken as written; an image is its alternative text');
});

test('blocks: headings, lists with tasks and nesting, quotes, code, rules and tables', async () => {
  const { parseMarkdown } = await parser;
  const blocks = parseMarkdown('# Title\n\nPara one\nstill.\n\n- a\n- [x] done\n  more\n  - nested\n- [ ] todo\n\n3. three\n4. four\n\n> quote\n> more\n\n```ts\nconst a = 1;\n\nb();\n```\n\n---\n\n| A | B |\n|---|--:|\n| 1 | **2** |\n| x \\| y | 4 |\n');
  assert.deepEqual(blocks.map(block => block.type), ['heading', 'paragraph', 'list', 'list', 'quote', 'code', 'rule', 'table']);
  const [, paragraph, bullets, numbers, quote, code, , table] = blocks;
  assert.deepEqual(paragraph.runs, [{ text: 'Para one still.' }]);
  assert.deepEqual(bullets.items.map(item => item.isDone), [undefined, true, false]);
  assert.deepEqual(bullets.items[1].blocks.map(block => block.type), ['paragraph', 'list'], 'an indented list belongs to its item');
  assert.deepEqual([numbers.isOrdered, numbers.start, numbers.items.length], [true, 3, 2]);
  assert.deepEqual(quote.blocks[0].runs, [{ text: 'quote more' }]);
  assert.deepEqual([code.language, code.text], ['ts', 'const a = 1;\n\nb();']);
  assert.deepEqual(table.align, ['start', 'end']);
  assert.deepEqual(table.rows[1][0], [{ text: 'x | y' }], 'an escaped bar stays in its cell');
  assert.deepEqual(parseMarkdown('<b>raw</b> text'), [{ type: 'paragraph', runs: [{ text: '<b>raw</b> text' }] }], 'raw HTML is text');
});

test('Note: headings at their levels, a link the app opens, tasks read by their labels, numbers in their list', async () => {
  const screen = await renderExample('Markdown/Markdown', 'Note');
  const headings = screen.queryAll(node => node.props.role === 'heading');
  assert.deepEqual(headings.map(node => [textOf(node), node.props.headingLevel]), [['Trip to the coast', 2], ['To pack', 3], ['On the way', 3]], '# starts at level 2');
  const link = screen.queryAll(node => node.tag === 'link')[0];
  assert.equal(textOf(link), 'ten minutes from the beach');
  screen.press(link);
  screen.getByText('Opened https://example.com/house');
  const lists = screen.queryAll(node => node.props.role === 'list');
  assert.deepEqual(lists.map(list => list.children.length), [4, 3]);
  const marks = screen.queryAll(node => node.props.role === 'img');
  assert.deepEqual(marks.map(mark => mark.props.accessibilityLabel), ['Done', 'Done', 'Not done', 'Not done']);
  assert.deepEqual(lists[1].children.map(item => item.children[0].text), ['1.', '2.', '3.']);
  assert.ok(lists[1].children.every(item => item.children[0].props.accessibilityHidden === 1), 'the list counts its items itself');
});

test('Reference: code keeps its lines in a sideways scroll; the table is a Table read with its headings; without onLinkClick nothing is a link', async () => {
  const screen = await renderExample('Markdown/Markdown', 'Reference');
  const code = screen.queryAll(node => node.tag === 'scroll')[0];
  assert.deepEqual([code.children[0].props.whiteSpace, code.children[0].props.fontFamily], ['pre', 'mono']);
  assert.match(code.children[0].text, /^const reply = [^\n]+\nif \(reply\.status === 401\) await signIn\(\);$/);
  const grid = screen.queryAll(node => node.props.role === 'grid')[0];
  assert.equal(grid.props.accessibilityLabel, 'Status, Meaning, Retry');
  const cell = screen.queryAll(node => node.props.role === 'gridcell' && node.props.accessibilityLabel === 'Retry: after 30 s');
  assert.equal(cell.length, 1);
  const plain = await renderPage(`
    import { Markdown } from '@liulidesign/core/Markdown';
    export function render() { return <page><Markdown source="See [the docs](https://example.com)." /></page>; }
  `);
  assert.equal(plain.queryAll(node => node.tag === 'link').length, 0);
  plain.getByText('See the docs.');
});
