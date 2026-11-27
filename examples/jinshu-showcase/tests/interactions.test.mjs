import { test } from 'node:test';
import assert from 'node:assert/strict';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { render, textOf } from '@lingxia/view/testing';
import {
  applyOps,
  callout,
  codeBlock,
  createDocument,
  createRegistry,
  details,
  heading,
  list,
  listItem,
  mathBlock,
  paragraph,
  plainText,
  table,
} from '@liulidesign/jinshu/schema';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const registry = createRegistry([]);
const desk = { sizeClass: 'regular', pointer: 'fine', language: 'zh-CN', viewport: { width: 1280, height: 900 } };
// A bare caret's formats live in the touch keyboard bar; a fine pointer formats a selection.
const touch = { sizeClass: 'compact', pointer: 'coarse', language: 'zh-CN', viewport: { width: 393, height: 852 } };

/** Mounts the controlled editor; emitted ops are applied and the screen re-rendered, like Logic would. */
async function mountEditor(doc, env = desk) {
  const state = { doc, batches: [] };
  const screen = await render(
    resolve(root, 'tests/editor-harness.page.tsx'),
    {
      env,
      data: { doc },
      actions: {
        ops(ops) {
          state.batches.push(ops);
          const res = applyOps(state.doc, ops, registry);
          const errors = res.problems.filter(p => p.severity === 'error');
          assert.deepEqual(errors, [], `ops applied cleanly: ${JSON.stringify(ops)}`);
          state.doc = res.doc;
          queueMicrotask(() => screen.rerender({ data: { doc: state.doc } }));
        },
      },
    },
    { root },
  );
  return { screen, state };
}

const flush = () => new Promise(r => setTimeout(r, 0));
// The wire carries `aria-label` as `accessibilityLabel` and `aria-hidden` as `accessibilityHidden`.
const labelOf = n => n.props.accessibilityLabel ?? n.props['aria-label'];
const byLabel = (screen, label) => {
  const hits = screen.queryAll(n => labelOf(n) === label);
  assert.ok(hits.length > 0, `no element labelled "${label}"`);
  return hits[0];
};
const hasLabel = (screen, label) => screen.queryAll(n => labelOf(n) === label).length > 0;
/** Presses the control whose visible text is `text` (Liuli Buttons render their label as text). */
/** Table header cells (rows[0] when headerRow) and body rows. */
const headerCells = t => (t.headerRow ? t.rows[0].cells : []);
const bodyRows = t => (t.headerRow ? t.rows.slice(1) : t.rows);

function pressText(screen, text) {
  let node = screen.getByText(text);
  while (node && node.tag !== 'button') node = node.parent;
  assert.ok(node, `no button around "${text}"`);
  screen.press(node);
}

test('table: pressing a column bar selects it and its toolbar deletes the column', async () => {
  const doc = createDocument({ blocks: [table(['题号', '错因', '知识点'], [['1', '审题', '极限'], ['2', '计算', '导数']], { id: 't' })] });
  const { screen, state } = await mountEditor(doc);
  screen.press(byLabel(screen, '选中第 2 列'));
  assert.ok(screen.getByText('第 2 列'), 'selection toolbar names the column');
  pressText(screen, '删除列');
  await flush();
  assert.deepEqual(headerCells(state.doc.blocks[0]).map(plainText), ['题号', '知识点']);
  assert.deepEqual(bodyRows(state.doc.blocks[0])[1].cells.map(plainText), ['2', '导数']);
});

test('table: row bar moves a row down and inserts below', async () => {
  const doc = createDocument({ blocks: [table(['A'], [['r1'], ['r2']], { id: 't' })] });
  const { screen, state } = await mountEditor(doc);
  screen.press(byLabel(screen, '选中第 1 行'));
  pressText(screen, '下移');
  await flush();
  assert.deepEqual(bodyRows(state.doc.blocks[0]).map(r => plainText(r.cells[0])), ['r2', 'r1']);
  pressText(screen, '下插');
  await flush();
  assert.equal(bodyRows(state.doc.blocks[0]).length, 3);
});

test('table: cells edit in place, Enter moves down and appends a row at the end', async () => {
  const doc = createDocument({ blocks: [table(['A', 'B'], [['', '']], { id: 't' })] });
  const { screen, state } = await mountEditor(doc);
  screen.press(byLabel(screen, '编辑单元格 1-1'));
  screen.type(byLabel(screen, '单元格 1-1'), '**极限**');
  screen.key(byLabel(screen, '单元格 1-1'), 'Enter');
  await flush();
  assert.equal(plainText(bodyRows(state.doc.blocks[0])[0].cells[0]), '极限');
  assert.equal(bodyRows(state.doc.blocks[0])[0].cells[0][0].bold, true, 'inline formatting kept');
  assert.equal(bodyRows(state.doc.blocks[0]).length, 2, 'Enter on the last row appended one');
  assert.ok(hasLabel(screen, '单元格 2-1'), 'caret moved to the new row');
});

test('paragraph: the editor grows with its lines and Bold at a caret wraps only the last word', async () => {
  const doc = createDocument({ blocks: [paragraph('Limits describe behaviour near a point', 'p')] });
  const { screen, state } = await mountEditor(doc, touch);
  screen.press(byLabel(screen, '编辑块 p'));
  const box = byLabel(screen, '编辑 paragraph');
  assert.equal(box.props.maxRows, 24, 'textarea auto-grows instead of collapsing to one row');
  screen.press(byLabel(screen, '粗体'));
  await flush();
  const runs = state.doc.blocks[0].runs;
  assert.equal(plainText(runs), 'Limits describe behaviour near a point');
  assert.deepEqual(runs.filter(r => r.bold).map(r => r.text), ['point']);
});

test('paragraph: opening and leaving an unchanged paragraph emits no ops; edits keep atoms and tones', async () => {
  const atom = { id: 'at_1', inline: 'x-demo-term', version: 1, attrs: { key: 'GPU' }, fallback: 'GPU' };
  const runs = [{ text: '2*3*4 ' }, atom, { text: ' warm', highlight: 'warning' }];
  const doc = createDocument({ blocks: [paragraph(runs, 'p1'), paragraph('next', 'p2')] });
  const { screen, state } = await mountEditor(doc);
  screen.press(byLabel(screen, '编辑块 p1'));
  screen.press(byLabel(screen, '编辑块 p2'));
  await flush();
  assert.deepEqual(state.batches, [], 'no ops for an unchanged paragraph');

  screen.press(byLabel(screen, '编辑块 p1'));
  const box = byLabel(screen, '编辑 paragraph');
  // The draft is the exact shorthand: escaped literals, an atom token, a toned highlight.
  screen.type(box, '2\\*3\\*4 ⟦GPU#at_1⟧=={warning} warm!==');
  screen.press(byLabel(screen, '编辑块 p2'));
  await flush();
  // The page runs in its own realm: compare as data.
  assert.deepEqual(JSON.parse(JSON.stringify(state.doc.blocks[0].runs)), [{ text: '2*3*4 ' }, atom, { text: ' warm!', highlight: 'warning' }]);
});

test('list: Enter on an empty item leaves the list for a new paragraph', async () => {
  const doc = createDocument({ blocks: [list('bullet', [listItem('第一项', { id: 'i1' }), listItem('', { id: 'i2' })], { id: 'l' })] });
  const { screen, state } = await mountEditor(doc);
  screen.press(byLabel(screen, '编辑列表项 2'));
  screen.key(byLabel(screen, '编辑列表项 2'), 'Enter');
  await flush();
  assert.equal(state.doc.blocks[0].items.length, 1);
  assert.equal(state.doc.blocks[1].type, 'paragraph');
});

test('list: items keep inline formatting outside edit mode', async () => {
  const doc = createDocument({ blocks: [list('bullet', [listItem([{ text: '重点', bold: true }, { text: ' 内容' }], { id: 'i1' })], { id: 'l' })] });
  const { screen } = await mountEditor(doc);
  const boldRuns = screen.queryAll(n => n.tag === 'span' && n.text === '重点');
  assert.ok(boldRuns.length > 0, 'bold run rendered as its own span');
});

test('callout: every paragraph is editable; a blank line makes a new one', async () => {
  const doc = createDocument({ blocks: [callout('tip', [paragraph('第一段', 'c1'), paragraph('第二段', 'c2')], { id: 'c', title: '易错点' })] });
  const { screen, state } = await mountEditor(doc);
  screen.press(byLabel(screen, '编辑提示块 c'));
  let body = byLabel(screen, '提示块正文');
  assert.equal(body.props.inputValue, '第一段\n\n第二段');
  screen.type(body, '第一段\n\n第二段改\n\n第三段');
  pressText(screen, '完成');
  await flush();
  const kids = state.doc.blocks[0].blocks;
  assert.deepEqual([...kids.map(b => plainText(b.runs))], ['第一段', '第二段改', '第三段']);
  assert.deepEqual([...kids.slice(0, 2).map(b => b.id)], ['c1', 'c2'], 'existing paragraph ids kept');
});

test('heading and paragraph markdown shortcuts convert in place', async () => {
  const doc = createDocument({ blocks: [paragraph('', 'p')] });
  const { screen, state } = await mountEditor(doc);
  screen.press(byLabel(screen, '编辑块 p'));
  screen.type(byLabel(screen, '编辑 paragraph'), '## 知识点');
  await flush();
  assert.equal(state.doc.blocks[0].type, 'heading');
  assert.equal(state.doc.blocks[0].level, 2);
});

/** Mounts the reader. */
async function mountReader(data) {
  const copied = [];
  const screen = await render(resolve(root, 'tests/reader-harness.page.tsx'), { env: desk, data, actions: { copy: c => copied.push(c) } }, { root });
  return { screen, copied };
}

test('reader: code copy appears only when the host can copy, and copies the code', async () => {
  const doc = createDocument({ blocks: [codeBlock('ts', 'const x = 1;\nconsole.log(x);', { id: 'code' })] });
  const without = await mountReader({ doc });
  assert.equal(without.screen.queryAll(n => textOf(n) === '复制' && n.tag === 'button').length, 0, 'no fake copy button');
  const withCopy = await mountReader({ doc, withCopy: true });
  pressText(withCopy.screen, '复制');
  assert.deepEqual(withCopy.copied, ['const x = 1;\nconsole.log(x);']);
  assert.ok(withCopy.screen.getByText('已复制'));
});

test('reader: code is syntax-highlighted in one selectable text', async () => {
  const doc = createDocument({ blocks: [codeBlock('ts', 'const n = 42; // answer', { id: 'code' })] });
  const { screen } = await mountReader({ doc });
  const spans = screen.queryAll(n => n.tag === 'span' && (n.text === 'const' || n.text === '42' || n.text === '// answer'));
  assert.equal(spans.length, 3, 'keyword, number and comment are coloured runs');
});

test('reader: selection survives long documents (budgeted, not switched off)', async () => {
  const blocks = Array.from({ length: 120 }, (_, i) => paragraph(`第 ${i + 1} 段`, `p${i}`));
  const { screen } = await mountReader({ doc: createDocument({ blocks }) });
  const selectable = screen.queryAll(n => n.tag === 'text' && n.props.userSelect === 'text');
  assert.ok(selectable.length > 0, 'a 120-block document still has selectable text');
  assert.ok(selectable.length <= 56, `stays within the page budget (${selectable.length})`);
});

test('reader: resume offers to continue at the last position', async () => {
  const doc = createDocument({ blocks: [heading(2, '第一节', 'h1'), paragraph('…', 'p1'), heading(2, '第二节：极限', 'h2')] });
  const { screen } = await mountReader({ doc, resume: { position: { blockId: 'h2', offset: 0 }, blocks: {}, media: {} } });
  assert.ok(screen.getByText('上次读到「第二节：极限」'));
  screen.press(screen.getByTestId('reader-resume'));
  assert.equal(screen.queryAll(n => n.props['data-testid'] === 'reader-resume').length, 0, 'bar dismissed after jumping');
});

test('reader: a closed details block holds its body until opened', async () => {
  const doc = createDocument({ blocks: [details('查看解析', [paragraph('因为分母趋于 0', 'd1')], { id: 'd' })] });
  const { screen } = await mountReader({ doc });
  const hiddenBody = () => screen.queryAll(n => n.props.accessibilityHidden === 1 && n.props.height === 0 && textOf(n).includes('因为分母趋于 0'));
  assert.equal(hiddenBody().length, 1, 'closed body is collapsed and hidden');
  screen.press(screen.getByText('查看解析').parent.parent);
  assert.equal(hiddenBody().length, 0, 'opens on press');
});

test('sticky ribbon: pinned at top and inserts a 3x4 table from the visual grid picker', async () => {
  const doc = createDocument({ blocks: [heading(2, '第一章', 'h1'), paragraph('正文', 'p1')] });
  const { screen, state } = await mountEditor(doc);
  const toolbar = byLabel(screen, '文档编辑工具栏');
  // The document bar is pinned together with the agent and review bars under it.
  assert.equal(toolbar.parent.props.position, 'sticky', 'top bar is pinned');
  screen.press(screen.getByTestId('editor-toolbar-add'));
  screen.press(byLabel(screen, '插入 3 行 4 列表格'));
  await flush();
  const tbl = state.doc.blocks[2];
  assert.equal(tbl.type, 'table');
  assert.equal(headerCells(tbl).length, 4);
  assert.equal(bodyRows(tbl).length, 3);
});

test('math block: editing keeps live rendered preview above LaTeX source', async () => {
  const doc = createDocument({ blocks: [mathBlock('\\frac{1}{2}', 'm1')] });
  const { screen, state } = await mountEditor(doc);
  screen.press(byLabel(screen, '编辑块 m1'));
  assert.ok(screen.getByText('上方实时预览排版 · 按 Esc 完成'), 'live preview hint is shown');
  const input = byLabel(screen, '编辑 math');
  screen.type(input, '\\frac{3}{4}');
  pressText(screen, '完成');
  await flush();
  assert.equal(state.doc.blocks[0].latex, '\\frac{3}{4}');
});
