import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample, zh } from '../testing/harness.mjs';

const desk = { sizeClass: 'regular', pointer: 'fine', viewport: { width: 1280, height: 900 } };
const labelled = (screen, label) => screen.queryAll(node => node.props.accessibilityLabel === label);

test('Draft: the toolbar and its controls are named in the page language', async () => {
  const en = await renderExample('JinshuEditor/JinshuEditor', 'Draft', { env: desk });
  assert.equal(labelled(en, 'Document toolbar').length, 1);
  assert.equal(labelled(en, 'Find and replace').length, 1);
  const cn = await renderExample('JinshuEditor/JinshuEditor', 'Draft', { env: { ...desk, ...zh.env } });
  assert.equal(labelled(cn, '文档编辑工具栏').length, 1);
  assert.equal(labelled(cn, '查找与替换').length, 1);
});

test('Draft: every block is drawn', async () => {
  const screen = await renderExample('JinshuEditor/JinshuEditor', 'Draft', { env: desk });
  for (const text of ['What a limit says', 'A limit describes where a function heads.']) assert.ok(screen.getByText(text), text);
});

test('ReadOnly: no toolbar', async () => {
  const screen = await renderExample('JinshuEditor/JinshuEditor', 'ReadOnly', { env: desk });
  assert.equal(labelled(screen, 'Document toolbar').length, 0);
  assert.ok(screen.getByText('What a limit says'));
});

// ---------------------------------------------------------------------------------------------
// The editor on a live source, through the same channel messages Logic sends.

import { renderPage } from '../testing/harness.mjs';
import { audio, callout, codeBlock, columns, createDocument, details, fileBlock, heading, image, list, listItem, mathBlock, paragraph, plainText, quote, table, video } from '../schema/index.ts';
import { createJinshuSource } from '../source/index.ts';
import { agentCall, toolsOver } from '../testing/agent.mjs';

const flush = () => new Promise(resolve => setTimeout(resolve, 0));
const labelOf = n => n.props.accessibilityLabel ?? n.props['aria-label'];
const hasLabel = (screen, label) => screen.queryAll(n => labelOf(n) === label).length > 0;
const byLabel = (screen, label) => {
  const hit = screen.queryAll(n => labelOf(n) === label)[0];
  assert.ok(hit, `no element labelled "${label}"`);
  return hit;
};
const hasText = (screen, text) => screen.queryAll(n => n.props.text === text || n.text === text).length > 0 || (() => {
  try {
    return Boolean(screen.getByText(text));
  } catch {
    return false;
  }
})();
function pressText(screen, text) {
  let node = screen.getByText(text);
  while (node && node.tag !== 'button') node = node.parent;
  assert.ok(node, `no button around "${text}"`);
  screen.press(node);
}

const lessonDoc = () =>
  createDocument({
    title: 'Week 3',
    blocks: [heading(1, 'Week 3', 'h'), paragraph('First paragraph.', 'p1'), paragraph('Second paragraph.', 'p2'), paragraph('Third.', 'p3')],
  });

/** The editor on `source`, which answers its `syncDoc` channel as Logic does; `ask` records its argument. */
async function mountLive(source, env = desk) {
  const asked = [];
  const screen = await renderPage(
    linked => `import { useView } from '@lingxia/view';
import { JinshuEditor } from ${JSON.stringify(`${linked}/src/JinshuEditor/JinshuEditor`)};
function Editor() {
  const { actions } = useView();
  if (!actions?.syncDoc) return <view />;
  return <JinshuEditor channel={actions.syncDoc} docKey="w3" onAskAgent={ids => void actions.ask({ ids })} hasLeaveGuard={false} data-testid="w3" />;
}
export function render() {
  return <page><Editor /></page>;
}
`,
    {
      env,
      actions: { ask: async args => void asked.push(JSON.parse(JSON.stringify(args))) },
      channels: { syncDoc: (channel, params) => source.serve(channel, params) },
    },
  );
  const settle = () => screen.settle();
  await settle();
  return { screen, asked, settle };
}

test('agent change: a bar names the agent and the count, and undoes that batch only', async () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc() });
  const { screen, settle } = await mountLive(source);
  assert.ok(screen.getByText('Second paragraph.'));
  const tools = toolsOver({ w3: source });
  await tools.call('jinshu_edit', { doc: 'w3', base: 1, ops: [{ op: 'edit', id: 'p2', find: 'Second', replace: 'Agent' }] });
  await settle();
  assert.ok(screen.getByText('Codex changed 1 block'));
  // The bar's View steps through the changes; no floating pill covers the text.
  assert.ok(hasText(screen, 'View'));
  assert.ok(!hasLabel(screen, 'Go to the next change'));
  pressText(screen, 'Undo this change');
  await settle();
  assert.equal(plainText(source.getState().doc.blocks[2].runs), 'Second paragraph.');
  assert.ok(!hasText(screen, 'Codex changed 1 block'));
});

test('agent change while away: the next page opening shows it', async () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc() });
  await toolsOver({ w3: source }).call('jinshu_edit', { doc: 'w3', base: 1, ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'Away' }] });
  const { screen } = await mountLive(source, { ...desk, ...zh.env });
  assert.ok(screen.getByText('你不在时，Codex 修改了 1 处'));
});

test('approval queue: a suggestion and a save request wait in one bar, no dialog', async () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc(), agentSave: 'confirm' });
  const { screen, settle } = await mountLive(source);
  const tools = toolsOver({ w3: source });
  await tools.call('jinshu_save', { doc: 'w3', summary: 'Tidy the intro' });
  await tools.call('jinshu_edit', { doc: 'w3', base: 1, mode: 'suggest', ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'Opening', note: 'Stronger start' }] });
  await settle();
  assert.ok(screen.getByText('2 items need you'));
  assert.equal(screen.queryAll(n => n.tag === 'dialog' || n.props.role === 'dialog').length, 0);
  assert.ok(screen.getByText('Codex asks to save'));
  assert.ok(screen.getByText('Tidy the intro'));
  pressText(screen, 'Decline');
  await settle();
  assert.equal(source.getState().pendingAgentSave, undefined);
  assert.ok(screen.getByText('Codex suggests: Stronger start'));
  const before = source.getState().revision;
  source.acceptAllSuggestions();
  await settle();
  assert.ok(source.getState().revision > before);
  assert.equal(plainText(source.getState().doc.blocks[1].runs), 'Opening paragraph.');
  assert.ok(!hasText(screen, '1 item needs you'));
});

test('agent activity: a writing mark names the agent and Stop refuses its next edit', async () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc() });
  const { screen, settle } = await mountLive(source);
  const tools = toolsOver({ w3: source });
  await tools.call('jinshu_edit', { doc: 'w3', base: 1, ops: [{ op: 'edit', id: 'p3', find: 'Third', replace: 'More' }] });
  await settle();
  assert.ok(screen.getByText('Codex is writing…'));
  pressText(screen, 'Stop');
  await settle();
  const refused = await tools.call('jinshu_edit', { doc: 'w3', base: source.getState().revision, ops: [{ op: 'delete', id: 'p3' }] }, agentCall());
  assert.equal(refused.code, 'STOPPED');
  assert.ok(!hasText(screen, 'Codex is writing…'));
});

test('phone: a media block has no overlay control until a tap selects it; the bar sits under it', async () => {
  const doc = createDocument({
    blocks: [paragraph('Intro', 'p'), fileBlock('./notes.pdf', 'notes.pdf', { id: 'f' }), paragraph('End', 'e')],
  });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source, { pointer: 'coarse', sizeClass: 'compact' });
  assert.ok(!hasLabel(screen, 'Block actions'), 'no menu over an unselected block');
  assert.equal(screen.queryAll(n => n.props.testID === 'w3-selection-bar' || n.props['data-testid'] === 'w3-selection-bar').length, 0);
  screen.press(byLabel(screen, 'Select block'));
  await settle();
  assert.ok(screen.getByTestId('w3-selection-bar'));
  for (const label of ['Move up', 'Move down', 'Delete', 'Block actions', 'Deselect']) assert.ok(hasLabel(screen, label), label);
  assert.ok(!hasLabel(screen, 'Select block'));
  screen.press(byLabel(screen, 'Deselect'));
  await settle();
  assert.equal(screen.queryAll(n => labelOf(n) === 'Deselect').length, 0);
});

test('block menu: "Ask agent" hands the block ids to the app', async () => {
  const doc = createDocument({ blocks: [paragraph('Intro', 'p'), fileBlock('./notes.pdf', 'notes.pdf', { id: 'f' })] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, asked, settle } = await mountLive(source, { pointer: 'coarse', sizeClass: 'compact' });
  screen.press(byLabel(screen, 'Select block'));
  await settle();
  screen.press(byLabel(screen, 'Block actions'));
  await settle();
  pressText(screen, 'Ask agent');
  await settle();
  assert.deepEqual(asked, [{ ids: ['f'] }]);
});

test('history: editor notes are words in the reader\'s language, and the person is "you"', async () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc() });
  source.apply({ batchId: 'b1', origin: { kind: 'human', id: 'editor' }, base: 1, ops: [{ op: 'delete', id: 'p3', noteWord: { key: '@jinshu.note.delete' } }] });
  const { screen, settle } = await mountLive(source, { ...desk, ...zh.env });
  screen.press(byLabel(screen, '修改记录'));
  await settle();
  assert.ok(screen.getByText('删除块'));
  assert.ok(screen.getByText('你'));
});

test('table: the grid scrolls inside its frame, in the reader and in the editor', async () => {
  const doc = createDocument({ blocks: [table(['Package', 'Path', 'Role'], [['jinshu', '@liulidesign/jinshu/schema', 'types']], { id: 't' })] });
  const phone = await mountLive(createJinshuSource({ key: 'w3', initialDoc: doc }), { pointer: 'coarse', sizeClass: 'compact' });
  assert.equal(phone.screen.getByTestId('w3-block-t-scroll').tag, 'scroll');
  const desktop = await mountLive(createJinshuSource({ key: 'w3', initialDoc: doc }));
  assert.equal(desktop.screen.getByTestId('w3-table-editor-t-scroll').tag, 'scroll');
  assert.ok(hasLabel(desktop.screen, 'Edit cell 1-2'));
});

test('details: the editor draws the reader\'s chevron, and "starts open" is a block-menu choice', async () => {
  const doc = createDocument({ blocks: [details('Why limits', [paragraph('Because.', 'dp')], { id: 'd' })] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source);
  for (const text of ['▸', '▾', 'Starts open', 'Starts closed']) assert.ok(!hasText(screen, text), `no inline "${text}"`);
  const toggle = byLabel(screen, 'Show or hide the contents');
  assert.equal(toggle.tag, 'button');
  const row = screen.queryAll(n => String(n.props.elementId ?? n.props.id ?? '').endsWith('-r-d'))[0];
  screen.fire(row, 'onPointerEnter');
  await settle();
  screen.press(byLabel(screen, 'Block actions'));
  await settle();
  pressText(screen, 'Start open');
  await settle();
  screen.press(byLabel(screen, 'Block actions'));
  await settle();
  assert.ok(hasText(screen, 'Start closed'));
});

test('code: the preview and the open editor share the header and the line numbers', async () => {
  const doc = createDocument({ blocks: [codeBlock('ts', 'const a = 1;\nconst b = 2;\nexport { a, b };', { filename: 'limits.ts', id: 'c' })] });
  const { screen, settle } = await mountLive(createJinshuSource({ key: 'w3', initialDoc: doc }));
  const numbers = () => screen.getByTestId('w3-block-c-numbers');
  assert.ok(hasText(screen, 'limits.ts · ts'));
  assert.equal(numbers().text ?? numbers().props.text, '1\n2\n3');
  pressText(screen, 'Edit');
  await settle();
  assert.ok(hasLabel(screen, 'Code'), 'the code field is open');
  assert.equal(numbers().text ?? numbers().props.text, '1\n2\n3');
});

test('table: an active cell offers Add row and Add column above the table', async () => {
  const doc = createDocument({ blocks: [table(['A'], [['1']], { id: 't' })] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source);
  assert.ok(!hasText(screen, 'Add row'), 'no toolbar on an idle table');
  screen.press(byLabel(screen, 'Edit cell 1-1'));
  await settle();
  pressText(screen, 'Add column');
  await settle();
  pressText(screen, 'Add row');
  await settle();
  const t = source.getState().doc.blocks[0];
  assert.equal(t.rows[0].cells.length, 2);
  assert.equal(t.rows.length, 3);
});

test('suggestion card: a marks-only change shows its new formatting, not the paragraph struck and retyped', async () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc() });
  const { screen, settle } = await mountLive(source);
  const tools = toolsOver({ w3: source });
  await tools.call('jinshu_edit', { doc: 'w3', base: 1, mode: 'suggest', ops: [{ op: 'edit', id: 'p1', find: 'First', replace: '<b>First</b>', note: 'Bold' }] });
  await settle();
  const sug = source.getState().suggestions[0];
  if (sug.kind === 'range') {
    assert.ok(hasText(screen, 'Formatting only'));
  } else {
    // A block suggestion keeps its op diff; the case this guards is the range form.
    assert.ok(hasText(screen, 'First'));
  }
  assert.ok(!hasText(screen, 'First paragraph.First paragraph.'));
});

test('paste: paragraphs or Markdown become blocks in one undo step; a soft line break stays text', async () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc() });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit block p1'));
  await settle();
  const pasted = screen.paste(byLabel(screen, 'Edit paragraph'), { text: 'Intro\n\n- one\n- two', selection: { start: 0, end: 'First paragraph.'.length } });
  assert.equal(pasted.prevented, true);
  await settle();
  const blocks = source.getState().doc.blocks;
  assert.deepEqual(blocks.map(b => b.type), ['heading', 'paragraph', 'list', 'paragraph', 'paragraph']);
  assert.equal(blocks[1].id, 'p1', 'the edited block keeps its id');
  assert.equal(plainText(blocks[1].runs), 'Intro');
  source.undo();
  assert.equal(plainText(source.getState().doc.blocks[1].runs), 'First paragraph.');
  assert.equal(source.getState().doc.blocks.length, 4);

  screen.press(byLabel(screen, 'Edit block p2'));
  await settle();
  assert.equal(screen.paste(byLabel(screen, 'Edit paragraph'), { text: 'line one\nline two' }).prevented, false, 'the field takes it as text');
  await settle();
  assert.equal(source.getState().doc.blocks.length, 4);
});

const pastedPage = `<html><head><meta charset="utf-8"><style>h2 { color: red }</style></head><body><!--StartFragment-->
<h2 style="margin:0">Limits</h2><p>A <b>limit</b> <a href="https://example.com/l">explained</a><script>alert(1)</script></p>
<ul><li>one</li><li>two</li></ul><!--EndFragment--></body></html>`;

test('paste HTML: blocks split the paragraph at the caret and go in between, in one undo step', async () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc() });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit block p1'));
  await settle();
  const at = 'First '.length;
  const res = screen.paste(byLabel(screen, 'Edit paragraph'), { text: 'Limits\nA limit explained\none\ntwo', html: pastedPage, selection: { start: at, end: at } });
  assert.equal(res.prevented, true);
  await settle();
  const blocks = source.getState().doc.blocks;
  assert.deepEqual(blocks.map(b => b.type), ['heading', 'paragraph', 'heading', 'paragraph', 'list', 'paragraph', 'paragraph', 'paragraph']);
  assert.equal(blocks[1].id, 'p1');
  assert.equal(plainText(blocks[1].runs), 'First ');
  assert.equal(blocks[2].level, 2);
  assert.equal(plainText(blocks[2].runs), 'Limits');
  assert.deepEqual(blocks[3].runs, [{ text: 'A ' }, { text: 'limit', bold: true }, { text: ' ' }, { text: 'explained', link: 'https://example.com/l' }]);
  assert.deepEqual(blocks[4].items.map(i => plainText(i.runs)), ['one', 'two']);
  assert.equal(plainText(blocks[5].runs), 'paragraph.', 'the text after the caret follows the pasted blocks');
  assert.ok(!JSON.stringify(blocks).includes('alert'), 'no script text');
  assert.ok(hasLabel(screen, 'Edit paragraph'), 'the caret stays in the text after the paste');
  source.undo();
  assert.deepEqual(source.getState().doc.blocks.map(b => b.id), ['h', 'p1', 'p2', 'p3']);
  assert.equal(plainText(source.getState().doc.blocks[1].runs), 'First paragraph.');
});

test('paste HTML: one paragraph goes in at the caret with its marks; an unsafe link keeps only its words', async () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc() });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit block p2'));
  await settle();
  const at = 'Second '.length;
  const html = '<meta charset="utf-8"><b style="font-weight:normal" id="docs-internal-guid-1"><span style="font-weight:700">bold</span> <a href="javascript:alert(1)">and</a> <em>new</em> </b>';
  assert.equal(screen.paste(byLabel(screen, 'Edit paragraph'), { text: 'bold and new ', html, selection: { start: at, end: at } }).prevented, true);
  await settle();
  const blocks = source.getState().doc.blocks;
  assert.equal(blocks.length, 4, 'no new block');
  assert.deepEqual(blocks[2].runs, [{ text: 'Second ' }, { text: 'bold', bold: true }, { text: ' and ' }, { text: 'new', italic: true }, { text: 'paragraph.' }]);
  assert.equal(byLabel(screen, 'Edit paragraph').props.inputValue, 'Second **bold** and *new*paragraph.');
});

test('paste HTML that adds nothing to its text goes in as typed shorthand', async () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc() });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit block p3'));
  await settle();
  const res = screen.paste(byLabel(screen, 'Edit paragraph'), { text: ' **more**', html: '<span> **more**</span>' });
  assert.equal(res.prevented, false);
  await settle();
  assert.equal(byLabel(screen, 'Edit paragraph').props.inputValue, 'Third. **more**');
});

test('markdown document: the menus and property editors offer only what Markdown holds', async () => {
  const doc = createDocument({ blocks: [paragraph('Intro', 'p'), callout('info', [paragraph('Note', 'cp')], { id: 'c' })] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc, format: 'markdown', media: { pick: async () => null } });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit block p'));
  await settle();
  screen.selectText(byLabel(screen, 'Edit paragraph'), 0, 5);
  await settle();
  assert.ok(hasLabel(screen, 'Bold'));
  for (const label of ['Underline', 'Highlight', 'Superscript', 'Subscript']) assert.ok(!hasLabel(screen, label), label);
  screen.type(byLabel(screen, 'Edit paragraph'), '/');
  await settle();
  for (const text of ['Image', 'Table', 'Callout']) assert.ok(hasText(screen, text), text);
  for (const text of ['Video', 'Audio', 'File', 'Toggle', 'Columns']) assert.ok(!hasText(screen, text), text);
  screen.press(byLabel(screen, 'Edit callout c'));
  await settle();
  assert.ok(!hasLabel(screen, 'Callout title'));
});

test('source mode: the document as Markdown, applied back as one batch of only the changed blocks', async () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc() });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit as Markdown'));
  await settle();
  const field = () => byLabel(screen, 'Markdown source');
  const text = field().props.inputValue;
  assert.match(text, /^# Week 3\n/);
  screen.type(field(), text.replace('Second paragraph.', 'Second, rewritten.').replace('# Week 3', '# Week 4'));
  await settle();
  screen.press(byLabel(screen, 'Back to blocks'));
  await settle();
  const state = source.getState();
  assert.equal(state.doc.title, 'Week 4');
  assert.deepEqual(state.doc.blocks.map(b => b.id), ['h', 'p1', 'p2', 'p3']);
  assert.equal(plainText(state.doc.blocks[2].runs), 'Second, rewritten.');
  assert.equal(state.history.length, 1, 'one batch');
  assert.ok(!hasLabel(screen, 'Markdown source'), 'back to blocks');
  source.undo();
  assert.equal(source.getState().doc.title, 'Week 3');
});

test('source mode: an agent change meanwhile refuses the text and keeps it', async () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc() });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit as Markdown'));
  await settle();
  await toolsOver({ w3: source }).call('jinshu_edit', { doc: 'w3', base: source.getState().revision, ops: [{ op: 'edit', id: 'p2', find: 'Second', replace: 'Agent' }] });
  await settle();
  const field = byLabel(screen, 'Markdown source');
  const mine = field.props.inputValue.replace('Second paragraph.', 'Mine.');
  screen.type(field, mine);
  await settle();
  screen.press(byLabel(screen, 'Back to blocks'));
  await settle();
  assert.equal(plainText(source.getState().doc.blocks[2].runs), 'Agent paragraph.');
  assert.ok(hasText(screen, 'Someone changed these blocks while you edited the text. Your text is kept.'));
  const kept = byLabel(screen, 'Markdown source');
  assert.equal(kept.props.inputValue, mine);
});

test('source mode: a document Markdown cannot hold offers it disabled, with the reason', async () => {
  const doc = createDocument({ blocks: [paragraph('Intro', 'p'), video('https://cdn.example/v.mp4', { id: 'v' })] });
  const { screen } = await mountLive(createJinshuSource({ key: 'w3', initialDoc: doc }));
  const toggle = byLabel(screen, 'Edit as Markdown');
  assert.equal(toggle.props.enabled, 0);
  assert.ok(toggle.props.ariaDescribedBy, 'the reason is its description');
});

test('input rules at line start: ordered from a number, checked task, alert callout, math, and a pipe row table', async () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc() });
  const { screen, settle } = await mountLive(source);
  const typeInto = async (id, text) => {
    screen.press(byLabel(screen, `Edit block ${id}`));
    await settle();
    screen.type(byLabel(screen, 'Edit paragraph'), text);
    await settle();
  };
  await typeInto('p1', '3) Third');
  let b = source.getState().doc.blocks[1];
  assert.equal(b.style, 'ordered');
  assert.equal(b.start, 3);
  await typeInto('p2', '- [x] Done');
  b = source.getState().doc.blocks[2];
  assert.equal(b.style, 'task');
  assert.equal(b.items[0].checked, true);
  await typeInto('p3', '> ');
  for (const step of ['> [', '> [!WARN', '> [!WARNING]']) {
    screen.type(byLabel(screen, 'Edit paragraph'), step);
    await settle();
    assert.equal(source.getState().doc.blocks[3].type, 'paragraph', `"${step}" could still become an alert`);
  }
  screen.type(byLabel(screen, 'Edit paragraph'), '> [!WARNING] Careful');
  await settle();
  b = source.getState().doc.blocks[3];
  assert.equal(b.type, 'callout');
  assert.equal(b.tone, 'warning');
  screen.press(byLabel(screen, 'Edit block h'));
  await settle();
  // A new empty paragraph for the last two rules.
  source.apply({ batchId: 'n', origin: { kind: 'human', id: 'editor' }, base: source.getState().revision, ops: [{ op: 'insert', at: { end: 'doc' }, blocks: [paragraph('', 'm'), paragraph('', 't')] }] });
  await settle();
  await typeInto('m', '$$');
  assert.equal(source.getState().doc.blocks[4].type, 'math');
  screen.press(byLabel(screen, 'Edit block t'));
  await settle();
  screen.type(byLabel(screen, 'Edit paragraph'), '| Week | Topic |');
  await settle();
  screen.key(byLabel(screen, 'Edit paragraph'), 'Enter');
  await settle();
  const grid = source.getState().doc.blocks[5];
  assert.equal(grid.type, 'table');
  assert.equal(grid.rows.length, 2);
  assert.equal(plainText(grid.rows[0].cells[1]), 'Topic');
});

test('lists: Enter continues a list; on an empty item it ends the list in one step', async () => {
  const doc = createDocument({ blocks: [list('bullet', [listItem('One', { id: 'a' }), listItem('', { id: 'b' }), listItem('Three', { id: 'c' })], { id: 'l' })] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit item 2'));
  await settle();
  screen.key(byLabel(screen, 'Edit item 2'), 'Enter');
  await settle();
  const blocks = source.getState().doc.blocks;
  assert.deepEqual(blocks.map(x => x.type), ['list', 'paragraph', 'list']);
  assert.equal(blocks[0].id, 'l');
  assert.deepEqual(blocks[0].items.map(i => i.id), ['a']);
  assert.deepEqual(blocks[2].items.map(i => i.id), ['c']);
  assert.equal(source.getState().history.length, 1);
});

test('line-start rules typed key by key: the caret follows into the new block with no starter text', async () => {
  const doc = createDocument({ blocks: [paragraph('', 'a'), paragraph('', 'b'), paragraph('', 'c'), paragraph('', 'd')] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc, format: 'markdown' });
  const { screen, settle } = await mountLive(source);
  // Each key goes to whichever field is open, appended to what it holds, as a keyboard types.
  const typeKeys = async (_field, text) => {
    for (const ch of text) {
      const fields = screen.queryAll(n => n.tag === 'input' || n.tag === 'textarea');
      assert.equal(fields.length, 1, `one open field before "${ch}"`);
      screen.type(fields[0], `${fields[0].props.inputValue ?? ''}${ch}`);
      await settle();
    }
  };

  screen.press(byLabel(screen, 'Edit block a'));
  await settle();
  await typeKeys('Edit paragraph', '- [x] ');
  let list = source.getState().doc.blocks[0];
  assert.equal(list.type, 'list');
  assert.equal(list.style, 'task', 'the box after "- " makes it a task list');
  assert.equal(list.items[0].checked, true);
  assert.deepEqual(list.items[0].runs, [], 'no starter text in the item');
  await typeKeys('Edit item 1', 'Tripod');
  screen.key(byLabel(screen, 'Edit item 1'), 'Enter');
  await settle();
  list = source.getState().doc.blocks[0];
  assert.equal(plainText(list.items[0].runs), 'Tripod');
  assert.ok(hasLabel(screen, 'Edit item 2'), 'Enter opened the next item');
  screen.key(byLabel(screen, 'Edit item 2'), 'Enter');
  await settle();
  assert.ok(hasLabel(screen, 'Edit paragraph'), 'Enter on the empty item leaves the list into a paragraph');

  screen.press(byLabel(screen, 'Edit block b'));
  await settle();
  await typeKeys('Edit paragraph', '> x');
  const quote = source.getState().doc.blocks.find(blk => blk.id === 'b');
  assert.equal(quote.type, 'quote');
  assert.equal(byLabel(screen, 'Quote text').props.inputValue, 'x', 'the quote opens holding what was typed');
  screen.blur(byLabel(screen, 'Quote text'));
  await settle();

  screen.press(byLabel(screen, 'Edit block c'));
  await settle();
  await typeKeys('Edit paragraph', '```py');
  assert.equal(source.getState().doc.blocks.find(blk => blk.id === 'c').type, 'paragraph', 'a fence waits for Enter');
  screen.key(byLabel(screen, 'Edit paragraph'), 'Enter');
  await settle();
  const code = source.getState().doc.blocks.find(blk => blk.id === 'c');
  assert.equal(code.type, 'code');
  assert.equal(code.language, 'py');
});

test('auto-save: text typed in an open paragraph saves after a pause, and the field stays open', async () => {
  const saves = [];
  const doc = createDocument({ blocks: [paragraph('Intro', 'p')] });
  const source = createJinshuSource({
    key: 'w3',
    initialDoc: doc,
    format: 'markdown',
    saveMode: 'auto',
    save: async (next, _base, file) => {
      saves.push(file.text);
      return { ok: true, version: `v${saves.length + 1}` };
    },
  });
  const { screen, settle } = await mountLive(source);
  assert.ok(!hasLabel(screen, 'Save'), 'an auto-saving document has no Save button');
  screen.press(byLabel(screen, 'Edit block p'));
  await settle();
  screen.type(byLabel(screen, 'Edit paragraph'), 'Intro, longer');
  await settle();
  assert.ok(hasText(screen, 'Saving…'), 'typed text counts as a change at once');
  assert.ok(!hasLabel(screen, 'Save'));
  await new Promise(resolve => setTimeout(resolve, 1700));
  await settle();
  assert.deepEqual(saves, ['Intro, longer\n']);
  assert.ok(hasLabel(screen, 'Edit paragraph'), 'still typing in the same field');
  assert.ok(hasText(screen, 'Saved'));
});

test('word count: CJK by character, other text by word, beside the save state', async () => {
  const doc = createDocument({ blocks: [paragraph('Hello brave world', 'p'), paragraph('锦书很好', 'q')] });
  const { screen } = await mountLive(createJinshuSource({ key: 'w3', initialDoc: doc }));
  assert.ok(hasText(screen, '7 words'));
});

test('source mode: a Markdown document shows its file as written, and an edit keeps the untouched bytes', async () => {
  const md = '---\ntags: [a]\n---\n\n# Notes\n\nA soft\nwrapped line.\n\n| A | B |\n|:--|--:|\n| 1 | 2 |\n\nLast.\n';
  const files = [];
  const source = createJinshuSource({ key: 'w3', format: 'markdown', initialText: md, save: async (d, b, f) => (files.push(f.text), { ok: true, version: 'v2' }) });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit as Markdown'));
  await settle();
  const field = byLabel(screen, 'Markdown source');
  assert.equal(field.props.inputValue, md);
  screen.type(field, md.replace('Last.', 'End.'));
  await settle();
  screen.press(byLabel(screen, 'Back to blocks'));
  await settle();
  await source.save();
  assert.equal(files[0], md.replace('Last.', 'End.'));
});

test('table: columns size to their content, the same in the reader and the editor, and never below their widest word', async () => {
  const doc = createDocument({
    blocks: [
      table(
        ['时间', '光圈', '快门', 'ISO', '备注'],
        [
          ['17:02', 'f/8', '1/250', '100', '逆光，云边发亮'],
          ['17:31', 'f/2', '1/60', '800', '余晖，偏暖，人物轮廓更柔和'],
        ],
        { id: 't' },
      ),
    ],
  });
  for (const env of [{ pointer: 'coarse', sizeClass: 'compact' }, undefined]) {
    const { screen } = await mountLive(createJinshuSource({ key: 'w3', initialDoc: doc }), env);
    const cells = screen.queryAll(n => n.props.grow !== undefined && n.props.minWidthCh !== undefined);
    const header = cells.slice(0, 5);
    const grows = header.map(c => c.props.grow);
    assert.ok(grows[4] > 3 * grows[0], `the notes column takes the room its text needs: ${grows}`);
    assert.ok(grows[3] <= grows[0], 'a short column stays short');
    for (let col = 0; col < 5; col++) assert.ok(cells.filter((_, i) => i % 5 === col).every(c => c.props.grow === grows[col]), 'every row splits the same way');
    const grid = screen.queryAll(n => n.props.widthPercent === 100 && n.props.minWidthCh !== undefined)[0];
    assert.equal(grid.props.minWidthCh, header.reduce((sum, c) => sum + c.props.minWidthCh, 0), 'it scrolls only below every column\'s minimum');
    assert.ok(grid.props.minWidthCh <= 60, 'five short columns fit a reading measure');
  }
});

test('loading: before the first snapshot the reader and the editor show a skeleton, and "unavailable" only when the channel fails', async () => {
  for (const component of ['JinshuView/JinshuView', 'JinshuEditor/JinshuEditor']) {
    const name = component.split('/')[1];
    const mount = channel => renderPage(
      linked => `import { useView } from '@lingxia/view';
import { ${name} } from ${JSON.stringify(`${linked}/src/${component}`)};
function Doc() {
  const { actions } = useView();
  if (!actions?.syncDoc) return <view />;
  return <${name} channel={actions.syncDoc} docKey="w3" hasLeaveGuard={false} />;
}
export function render() { return <page><Doc /></page>; }
`,
      { env: desk, channels: { syncDoc: channel } },
    );
    const waiting = await mount(() => {});
    await waiting.settle();
    assert.ok(!hasText(waiting, "This document isn't available"), `${name}: loading is not "unavailable"`);
    assert.equal(waiting.queryAll(n => n.props.accessibilityLabel === 'Loading document…').length, 1, `${name}: the skeleton says it is loading`);
    const failed = await mount(channel => void setTimeout(() => channel.close({ code: 'E_NOT_FOUND', message: 'no such document' }), 0));
    await failed.settle();
    assert.ok(hasText(failed, "This document isn't available"), `${name}: a failed channel says so`);
  }
});

test('toolbar inserts: a wide bar offers common blocks; an empty line becomes the block, an open block saves and closes first', async () => {
  const doc = createDocument({ blocks: [heading(2, 'Notes', 'h'), paragraph('Intro', 'p'), paragraph('', 'e')] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit block e'));
  await settle();
  screen.press(byLabel(screen, 'Insert Quote'));
  await settle();
  assert.equal(source.getState().doc.blocks[2].type, 'quote', 'the empty line became the quote');
  screen.type(byLabel(screen, 'Quote text'), 'Said');
  await settle();
  screen.press(byLabel(screen, 'Insert Table'));
  await settle();
  const blocks = source.getState().doc.blocks;
  assert.deepEqual(blocks.map(b => b.type), ['heading', 'paragraph', 'quote', 'table']);
  assert.equal(plainText(blocks[2].blocks[0].runs), 'Said', 'the open quote saved, with no starter text');
  assert.ok(!hasLabel(screen, 'Quote text'), 'and closed, so the next field takes focus');

  const phone = await mountLive(createJinshuSource({ key: 'w3', initialDoc: doc }), { sizeClass: 'compact', pointer: 'coarse', viewport: { width: 390, height: 844 } });
  assert.ok(!hasLabel(phone.screen, 'Insert Quote'), 'a phone keeps the top bar short');
  assert.ok(hasLabel(phone.screen, 'Insert'));
});

test('auto-save: text typed in an open quote saves after a pause, and the quote stays open', async () => {
  const saves = [];
  const doc = createDocument({ blocks: [quote([paragraph('Said')], 'q')] });
  const source = createJinshuSource({
    key: 'w3',
    initialDoc: doc,
    format: 'markdown',
    saveMode: 'auto',
    save: async (_next, _base, file) => (saves.push(file.text), { ok: true, version: `v${saves.length + 1}` }),
  });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit quote q'));
  await settle();
  screen.type(byLabel(screen, 'Quote text'), 'Said twice');
  await settle();
  await new Promise(resolve => setTimeout(resolve, 1700));
  await settle();
  assert.deepEqual(saves, ['> Said twice\n']);
  assert.ok(hasLabel(screen, 'Quote text'), 'still typing in the quote');
});

test('toolbars: every button that shows only an icon names itself in a hint', async () => {
  const doc = createDocument({ blocks: [paragraph('Intro', 'p'), table(['A', 'B'], [['1', '2']], { id: 't' })] });
  const { screen, settle } = await mountLive(createJinshuSource({ key: 'w3', initialDoc: doc }));
  screen.press(byLabel(screen, 'Edit block p'));
  await settle();
  // The format bar shows over selected text.
  screen.selectText(byLabel(screen, 'Edit paragraph'), 0, 5);
  await settle();
  const inToolbar = node => {
    for (let at = node.parent; at; at = at.parent) if (at.props.role === 'toolbar' || at.props.accessibilityRole === 'toolbar') return true;
    return false;
  };
  const hasWords = node => (node.children ?? []).some(child => child.tag === 'text' && /\S/.test(child.props.text ?? child.text ?? ''));
  const iconOnly = screen.queryAll(node => node.tag === 'button' && inToolbar(node) && labelOf(node) && !hasWords(node));
  assert.ok(iconOnly.length >= 15, `the document and format bars hold their icon buttons (found ${iconOnly.length})`);
  const bare = iconOnly.filter(node => !node.props.interestFor).map(labelOf);
  assert.deepEqual(bare, [], 'each shows its name while a pointer rests on it');
  for (const label of ['Undo', 'Insert Quote', 'Insert', 'Bold', 'Highlight']) assert.ok(byLabel(screen, label).props.interestFor, label);
});

test('toolbar inserts with no block open land below the block last worked in, not at the end', async () => {
  const doc = createDocument({ blocks: [heading(2, 'Notes', 'h'), paragraph('Intro', 'p'), paragraph('Last', 'z')] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit block h'));
  await settle();
  screen.key(byLabel(screen, 'Edit heading'), 'Escape');
  await settle();
  assert.ok(!hasLabel(screen, 'Edit heading'), 'nothing is open');
  screen.press(byLabel(screen, 'Insert Table'));
  await settle();
  assert.deepEqual(source.getState().doc.blocks.map(b => b.type), ['heading', 'table', 'paragraph', 'paragraph']);
});

test('a block only started (a quote, list or code block closed with nothing in it) goes; one with text stays', async () => {
  const doc = createDocument({ blocks: [paragraph('Intro', 'p')] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source);
  for (const [button, field] of [['Insert Quote', 'Quote text'], ['Insert Bulleted list', 'Edit item 1']]) {
    screen.press(byLabel(screen, 'Edit block p'));
    await settle();
    screen.press(byLabel(screen, button));
    await settle();
    assert.equal(source.getState().doc.blocks.length, 2, `${button} made a block`);
    screen.blur(byLabel(screen, field));
    await settle();
    assert.deepEqual(source.getState().doc.blocks.map(b => b.id), ['p'], `${button}: left empty, it went`);
  }
  screen.press(byLabel(screen, 'Edit block p'));
  await settle();
  screen.press(byLabel(screen, 'Insert Quote'));
  await settle();
  screen.type(byLabel(screen, 'Quote text'), 'Kept');
  screen.key(byLabel(screen, 'Quote text'), 'Escape');
  await settle();
  assert.deepEqual(source.getState().doc.blocks.map(b => b.type), ['paragraph', 'quote']);
});

test('format buttons act on the selected words, keep them selected, show which marks are on, and come off again', async () => {
  const doc = createDocument({ blocks: [paragraph('Jinshu stores documents as JSON', 'p')] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit block p'));
  await settle();
  const field = () => byLabel(screen, 'Edit paragraph');
  const bar = () => screen.queryAll(n => n.testId === 'w3-format-bar')[0];
  assert.equal(bar(), undefined, 'no bar over the page while only a caret is in the text');
  screen.selectText(field(), 14, 23);
  await settle();
  assert.ok(bar(), 'selecting words brings the bar');
  assert.match(bar().props.anchorTo, /:selection$/, 'and it rides on the selection');
  assert.equal(byLabel(screen, 'Bold').props.keepsFocus, 1, 'a press keeps the selection in the text');
  screen.press(byLabel(screen, 'Bold'));
  await settle();
  assert.equal(field().props.inputValue, 'Jinshu stores **documents** as JSON');
  assert.deepEqual(Array.from(field().props.selectionRequest).slice(1), [16, 25], 'the same words stay selected');
  assert.equal(byLabel(screen, 'Bold').props.statePressed, 1);
  assert.equal(byLabel(screen, 'Italic').props.statePressed, 0);
  assert.deepEqual(source.getState().doc.blocks[0].runs.map(r => r.text), ['Jinshu stores ', 'documents', ' as JSON']);
  screen.press(byLabel(screen, 'Bold'));
  await settle();
  assert.equal(field().props.inputValue, 'Jinshu stores documents as JSON');
  assert.equal(byLabel(screen, 'Bold').props.statePressed, 0);
  screen.selectText(field(), 9);
  await settle();
  assert.equal(bar(), undefined, 'a caret again: the bar goes');
});

test('a format with only a caret marks the word it is in; Enter breaks the line at the caret', async () => {
  const doc = createDocument({ blocks: [paragraph('Hello world', 'p')] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source, { pointer: 'coarse', sizeClass: 'compact' });
  screen.press(byLabel(screen, 'Edit block p'));
  await settle();
  const field = () => byLabel(screen, 'Edit paragraph');
  screen.selectText(field(), 8);
  screen.press(byLabel(screen, 'Italic'));
  await settle();
  assert.equal(field().props.inputValue, 'Hello *world*');
  assert.deepEqual(Array.from(field().props.selectionRequest).slice(1), [9, 9], 'the caret stays where it was in the word');
  screen.selectText(field(), 9);
  screen.key(field(), 'Enter');
  await settle();
  const blocks = source.getState().doc.blocks;
  assert.deepEqual(blocks.map(b => b.runs.map(r => r.text).join('')), ['Hello wo', 'rld']);
  assert.deepEqual(blocks.map(b => b.runs.at(-1).italic), [true, true], 'both halves stay italic');
  assert.equal(field().props.inputValue, '*rld*', 'the caret moved on with the rest of the line');
  assert.deepEqual(Array.from(field().props.selectionRequest).slice(1), [0, 0]);
});

test('a link or an inline formula takes the selected text as its own', async () => {
  const doc = createDocument({ blocks: [paragraph('see the docs here', 'p')] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit block p'));
  await settle();
  const field = () => byLabel(screen, 'Edit paragraph');
  screen.selectText(field(), 8, 12);
  await settle();
  screen.press(byLabel(screen, 'Link'));
  await settle();
  assert.equal(byLabel(screen, 'Link text').props.inputValue, 'docs', 'the selection fills the link text');
  screen.type(byLabel(screen, 'Link address'), 'https://x.dev');
  await settle();
  screen.press(screen.queryAll(n => n.tag === 'button' && labelOf(n) === 'Insert' && n.props.enabled !== 0).at(-1));
  await settle();
  assert.equal(field().props.inputValue, 'see the [docs](https://x.dev) here');
  screen.type(field(), 'see the [docs](https://x.dev) here, area x^2');
  const at = field().props.inputValue.indexOf('x^2');
  screen.selectText(field(), at, at + 3);
  await settle();
  screen.press(byLabel(screen, 'Equation'));
  await settle();
  assert.equal(byLabel(screen, 'LaTeX source').props.inputValue, 'x^2', 'the selection fills the formula');
  screen.press(byLabel(screen, 'Inline'));
  await settle();
  assert.equal(field().props.inputValue, 'see the [docs](https://x.dev) here, area $x^2$', 'the formula takes the selection’s place');
  assert.equal(source.getState().doc.blocks.length, 1);
});

test('find and replace: Replace All changes every match Find counts, any case, as plain text keeping the marks', async () => {
  const doc = createDocument({
    title: 'Week 3',
    blocks: [{ ...paragraph('', 'p1'), runs: [{ text: 'Limit and ' }, { text: 'limit', bold: true }, { text: '.' }] }, codeBlock('js', 'const LIMIT = 1;', { id: 'k' })],
  });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Find and replace'));
  await settle();
  screen.type(byLabel(screen, 'Find text'), 'limit');
  screen.type(byLabel(screen, 'Replace with'), 'a<b> & c');
  await settle();
  pressText(screen, 'Replace all');
  await settle();
  const [p, k] = source.getState().doc.blocks;
  assert.equal(plainText(p.runs), 'a<b> & c and a<b> & c.');
  assert.ok(p.runs.some(run => run.text === 'a<b> & c' && run.bold), JSON.stringify(p.runs));
  assert.equal(k.code, 'const a<b> & c = 1;');
});

test('source mode: deleting the front matter removes it, and replacing every block leaves no blank paragraph', async () => {
  const source = createJinshuSource({ key: 'w3', format: 'markdown', initialText: '---\ntag: a\n---\n# Week 3\n\nFirst.\n\nSecond.\n' });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit as Markdown'));
  await settle();
  screen.type(byLabel(screen, 'Markdown source'), '# Week 3\n\nOther.\n');
  await settle();
  screen.press(byLabel(screen, 'Back to blocks'));
  await settle();
  const { doc } = source.getState();
  assert.equal(doc.meta?.frontMatter, undefined);
  assert.deepEqual(doc.blocks.map(b => plainText(b.runs)), ['Other.']);
});

test('block menu: duplicating an audio block gives its transcript new ids', async () => {
  const doc = createDocument({ blocks: [paragraph('Intro', 'p'), audio('https://cdn.example/a.mp3', { id: 'a', title: 'Recap', transcript: [paragraph('Said.', 't1')] })] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source, { pointer: 'coarse', sizeClass: 'compact' });
  screen.press(byLabel(screen, 'Select block'));
  await settle();
  screen.press(byLabel(screen, 'Block actions'));
  await settle();
  pressText(screen, 'Duplicate block');
  await settle();
  const { doc: after, problems } = source.getState();
  assert.deepEqual(problems.filter(p => p.severity === 'error'), []);
  const copies = after.blocks.filter(b => b.type === 'audio');
  assert.equal(copies.length, 2);
  assert.notEqual(copies[1].transcript[0].id, 't1');
  assert.equal(plainText(copies[1].transcript[0].runs), 'Said.');
});

test('auto-save: an open fold, column paragraph and formula save what they hold, and stay open', async () => {
  const doc = createDocument({
    blocks: [
      details('Hint', [paragraph('Look closer.')], { id: 'd' }),
      columns([{ id: 'c1', blocks: [paragraph('Left', 'l')] }, { id: 'c2', blocks: [paragraph('Right', 'r')] }], { id: 'cols' }),
      mathBlock('x^2', 'm'),
    ],
  });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc, saveMode: 'auto', save: async () => ({ ok: true, version: 'v2' }) });
  const { screen, settle } = await mountLive(source);
  const pause = async () => {
    await new Promise(resolve => setTimeout(resolve, 1700));
    await settle();
  };
  screen.type(byLabel(screen, 'Summary'), 'Hint, again');
  await settle();
  await pause();
  assert.equal(plainText(source.getState().doc.blocks[0].summary), 'Hint, again');

  screen.press(byLabel(screen, 'Edit column block l'));
  await settle();
  screen.type(byLabel(screen, 'Edit paragraph in column'), 'Left side');
  await settle();
  await pause();
  assert.equal(plainText(source.getState().doc.blocks[1].columns[0].blocks[0].runs), 'Left side');
  assert.ok(hasLabel(screen, 'Edit paragraph in column'), 'still typing in the column');

  screen.press(byLabel(screen, 'Edit block m'));
  await settle();
  screen.type(byLabel(screen, 'Edit math'), 'x^3');
  await settle();
  await pause();
  assert.equal(source.getState().doc.blocks[2].latex, 'x^3');
  assert.ok(hasLabel(screen, 'Edit math'), 'still typing in the formula');
});

const agentApply = (source, ops) => source.apply({ batchId: `a${Math.random().toString(36).slice(2, 8)}`, origin: { kind: 'agent', id: 'codex', label: 'Codex' }, base: source.getState().revision, ops });

test('details: an untouched fold follows the document, and a toolbar insert does not write old text over a change made elsewhere', async () => {
  const doc = createDocument({ blocks: [details('Why limits', [paragraph('Because.', 'dp')], { id: 'd', defaultOpen: true }), paragraph('End', 'e')] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source);
  agentApply(source, [{ op: 'replace', id: 'd', blocks: [details('Why limits matter', [paragraph('Because they do.', 'dp')], { id: 'd', defaultOpen: true })] }]);
  await settle();
  assert.equal(byLabel(screen, 'Summary').props.inputValue, 'Why limits matter');
  screen.press(byLabel(screen, 'Insert Table'));
  await settle();
  const fold = source.getState().doc.blocks.find(b => b.id === 'd');
  assert.equal(plainText(fold.summary), 'Why limits matter', 'the insert left the change made elsewhere alone');
  assert.equal(plainText(fold.blocks[0].runs), 'Because they do.');

  // Typed, sent, then changed elsewhere: the field shows the newer text again.
  screen.type(byLabel(screen, 'Summary'), 'Mine');
  screen.blur(byLabel(screen, 'Summary'));
  await settle();
  assert.equal(plainText(source.getState().doc.blocks.find(b => b.id === 'd').summary), 'Mine');
  agentApply(source, [{ op: 'text', target: { blockId: 'd', field: 'summary' }, from: 0, to: 4, insert: [{ text: 'Theirs' }] }]);
  await settle();
  assert.equal(byLabel(screen, 'Summary').props.inputValue, 'Theirs');
});

test('turn into: typed text survives the change, and a type that would lose content is offered disabled, saying why', async () => {
  const phone = { pointer: 'coarse', sizeClass: 'compact' };
  const doc = createDocument({ blocks: [paragraph('Plain', 'p'), { ...paragraph('', 'b'), runs: [{ text: 'Very ' }, { text: 'bold', bold: true }] }] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source, phone);
  screen.press(byLabel(screen, 'Edit block p'));
  await settle();
  screen.type(byLabel(screen, 'Edit paragraph'), 'Plain, retyped');
  await settle();
  screen.press(screen.getByTestId('w3-turn-into'));
  await settle();
  pressText(screen, 'Heading 2');
  await settle();
  const made = source.getState().doc.blocks[0];
  assert.equal(made.type, 'heading');
  assert.equal(plainText(made.runs), 'Plain, retyped');
  assert.equal(byLabel(screen, 'Edit heading').props.inputValue, 'Plain, retyped', 'the field keeps what was typed');
  assert.equal(source.getState().history.length, 1, 'the text and the change are one step');

  screen.press(byLabel(screen, 'Edit block b'));
  await settle();
  screen.press(screen.getByTestId('w3-turn-into'));
  await settle();
  let item = screen.getByText('Heading 2');
  while (item && item.tag !== 'button') item = item.parent;
  assert.equal(item.props.enabled, 0, 'bold cannot become a heading');
  assert.ok(hasText(screen, 'Not possible without losing content'));
});

test('find and replace: text in lists, tables, quotes, callouts, folds and columns is found and replaced', async () => {
  const doc = createDocument({
    blocks: [
      list('bullet', [listItem('A limit item', { id: 'i' })], { id: 'l' }),
      table(['Limit'], [['no limit here']], { id: 't' }),
      quote([paragraph('Quoted limit', 'qp')], 'q'),
      callout('tip', [paragraph('Body limit', 'cp')], { title: 'Limit title', id: 'c' }),
      details('Limit summary', [paragraph('Folded limit', 'dp')], { id: 'd' }),
      columns([{ id: 'c1', blocks: [paragraph('Left limit', 'lp')] }, { id: 'c2', blocks: [paragraph('Right', 'rp')] }], { id: 'cols' }),
    ],
  });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Find and replace'));
  await settle();
  screen.type(byLabel(screen, 'Find text'), 'limit');
  screen.type(byLabel(screen, 'Replace with'), 'bound');
  await settle();
  assert.ok(hasText(screen, '9 matches'));
  pressText(screen, 'Replace all');
  await settle();
  const [l, t, q, c, d, cols] = source.getState().doc.blocks;
  assert.equal(plainText(l.items[0].runs), 'A bound item');
  assert.deepEqual(t.rows.map(row => plainText(row.cells[0])), ['bound', 'no bound here']);
  assert.equal(plainText(q.blocks[0].runs), 'Quoted bound');
  assert.equal(plainText(c.title), 'bound title');
  assert.equal(plainText(c.blocks[0].runs), 'Body bound');
  assert.equal(plainText(d.summary), 'bound summary');
  assert.equal(plainText(d.blocks[0].runs), 'Folded bound');
  assert.equal(plainText(cols.columns[0].blocks[0].runs), 'Left bound');
});

test('find and replace: an open field with a match takes the replacement, and leaving it keeps it', async () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc() });
  const { screen, settle } = await mountLive(source);
  screen.press(byLabel(screen, 'Edit block p1'));
  await settle();
  screen.type(byLabel(screen, 'Edit paragraph'), 'First paragraph, typed.');
  await settle();
  screen.press(byLabel(screen, 'Find and replace'));
  await settle();
  screen.type(byLabel(screen, 'Find text'), 'paragraph');
  screen.type(byLabel(screen, 'Replace with'), 'line');
  await settle();
  pressText(screen, 'Replace all');
  await settle();
  assert.equal(byLabel(screen, 'Edit paragraph').props.inputValue, 'First line, typed.');
  screen.blur(byLabel(screen, 'Edit paragraph'));
  await settle();
  assert.deepEqual(source.getState().doc.blocks.slice(1, 3).map(b => plainText(b.runs)), ['First line, typed.', 'Second line.']);
});

test('columns: Enter starts a paragraph below, the bar switches 2 and 3 columns without losing blocks, and a formula edits in place', async () => {
  const doc = createDocument({
    blocks: [columns([{ id: 'c1', blocks: [paragraph('Left side', 'l')] }, { id: 'c2', blocks: [paragraph('Right', 'r'), mathBlock('x^2', 'm')] }], { id: 'cols' })],
  });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source);
  const cols = () => source.getState().doc.blocks[0].columns;
  screen.press(byLabel(screen, 'Edit column block l'));
  await settle();
  screen.selectText(byLabel(screen, 'Edit paragraph in column'), 4);
  screen.key(byLabel(screen, 'Edit paragraph in column'), 'Enter');
  await settle();
  assert.deepEqual(cols()[0].blocks.map(b => plainText(b.runs)), ['Left', ' side']);
  assert.equal(byLabel(screen, 'Edit paragraph in column').props.inputValue, ' side', 'the caret moved on with the rest');

  pressText(screen, 'Remove');
  await settle();
  assert.deepEqual(cols()[0].blocks.map(b => plainText(b.runs)), ['Left']);

  screen.press(byLabel(screen, 'Edit column block r'));
  await settle();
  pressText(screen, '3 columns');
  await settle();
  assert.equal(cols().length, 3);
  pressText(screen, 'Widen column 1');
  await settle();
  assert.deepEqual(source.getState().doc.blocks[0].widths, [2, 1, 1]);
  pressText(screen, '2 columns');
  await settle();
  assert.equal(cols().length, 2);
  assert.deepEqual(cols()[1].blocks.map(b => b.id).slice(0, 2), ['r', 'm'], 'the third column joined the second');
  assert.equal(source.getState().doc.blocks[0].widths, undefined);

  screen.press(byLabel(screen, 'Edit column block m'));
  await settle();
  screen.type(byLabel(screen, 'Edit math'), 'x^3');
  pressText(screen, 'Done');
  await settle();
  assert.equal(cols()[1].blocks.find(b => b.id === 'm').latex, 'x^3');
});

test('desktop: the toolbar puts a link or an equation at a bare caret', async () => {
  const doc = createDocument({ blocks: [paragraph('see here', 'p')] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source);
  const toolbarLink = () => screen.queryAll(n => labelOf(n) === 'Link' && n.tag === 'button')[0];
  assert.equal(toolbarLink().props.enabled, 0, 'nothing is being edited');
  screen.press(byLabel(screen, 'Edit block p'));
  await settle();
  const field = () => byLabel(screen, 'Edit paragraph');
  screen.selectText(field(), 4);
  await settle();
  assert.equal(screen.queryAll(n => n.testId === 'w3-format-bar')[0], undefined, 'no floating bar at a caret');
  screen.press(toolbarLink());
  await settle();
  screen.type(byLabel(screen, 'Link text'), 'docs');
  screen.type(byLabel(screen, 'Link address'), 'https://x.dev');
  await settle();
  screen.press(screen.queryAll(n => n.tag === 'button' && labelOf(n) === 'Insert' && n.props.enabled !== 0).at(-1));
  await settle();
  assert.equal(field().props.inputValue, 'see [docs](https://x.dev)here');
});

test('agent change bar: the count, View and Undo all mean the latest batch; dismissing it shows the one before', async () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc() });
  const { screen, settle } = await mountLive(source);
  const tools = toolsOver({ w3: source });
  await tools.call('jinshu_edit', { doc: 'w3', base: 1, ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'One' }] });
  await tools.call('jinshu_edit', { doc: 'w3', base: source.getState().revision, ops: [{ op: 'edit', id: 'p2', find: 'Second', replace: 'Two' }, { op: 'delete', id: 'p3' }] });
  await settle();
  assert.ok(hasText(screen, 'Codex changed 2 blocks'), 'the edited block and the deleted one');
  pressText(screen, 'Undo this change');
  await settle();
  assert.deepEqual(source.getState().doc.blocks.map(b => plainText(b.runs)), ['Week 3', 'One paragraph.', 'Second paragraph.', 'Third.']);
  assert.ok(hasText(screen, 'Codex changed 1 block'));
});

test('inspector: a change made elsewhere shows in an untouched field, and leaving it does not undo that change', async () => {
  const doc = createDocument({ blocks: [paragraph('Intro', 'p'), image('https://cdn.example/a.png', { alt: 'A chart', id: 'img' })] });
  const source = createJinshuSource({ key: 'w3', initialDoc: doc });
  const { screen, settle } = await mountLive(source, { pointer: 'coarse', sizeClass: 'compact' });
  screen.press(byLabel(screen, 'Select block'));
  await settle();
  screen.press(byLabel(screen, 'Edit properties'));
  await settle();
  assert.equal(byLabel(screen, 'Alt text').props.inputValue, 'A chart');
  await toolsOver({ w3: source }).call('jinshu_edit', { doc: 'w3', base: source.getState().revision, ops: [{ op: 'update', id: 'img', attrs: { alt: 'Sales by month' } }] });
  await settle();
  assert.equal(byLabel(screen, 'Alt text').props.inputValue, 'Sales by month');
  screen.blur(byLabel(screen, 'Alt text'));
  await settle();
  assert.equal(source.getState().doc.blocks[1].alt, 'Sales by month');
});

test('save state: a manually saved document shows Save while dirty, from the channel alone', async () => {
  let finish;
  const source = createJinshuSource({ key: 'w3', initialDoc: lessonDoc(), save: () => new Promise(resolve => (finish = resolve)) });
  const { screen, settle } = await mountLive(source);
  assert.ok(hasText(screen, 'Saved'));
  source.apply({ batchId: 'x', origin: { kind: 'agent', id: 'a' }, base: source.getState().revision, ops: [{ op: 'edit', id: 'p3', find: 'Third', replace: 'Last' }] });
  await settle();
  assert.ok(hasLabel(screen, 'Save'), 'dirty without any page flag');
  screen.press(byLabel(screen, 'Save'));
  await settle();
  assert.ok(hasText(screen, 'Saving…'));
  finish({ ok: true, version: 'v2' });
  await flush();
  await settle();
  assert.ok(hasText(screen, 'Saved'));
  assert.ok(!hasLabel(screen, 'Save'));
});

test('in-page link to an inline atom: the editor scrolls to the block that holds it, like the reader', async () => {
  const screen = await renderPage(
    linked => `import { JinshuEditor } from ${JSON.stringify(`${linked}/src/JinshuEditor/JinshuEditor`)};
import { useJinshuNav } from ${JSON.stringify(`${linked}/src/internal/nav`)};
import { createDocument, inlineAtom, paragraph, run } from ${JSON.stringify(`${linked}/src/schema/index`)};
const doc = createDocument({ blocks: [
  paragraph([run('See '), run('the ref', { link: '#a1' })], 'p1'),
  paragraph([run('Held '), inlineAtom('x-app-ref', { to: 1 }, 'ref', { id: 'a1' })], 'p2'),
] });
export function render() {
  const nav = useJinshuNav();
  return <page><text>{'target:' + String(nav.state.targetBlockId)}</text><JinshuEditor doc={doc} nav={nav} hasLeaveGuard={false} /></page>;
}
`,
    { env: desk },
  );
  screen.press(screen.queryAll(n => n.tag === 'link')[0]);
  await screen.settle();
  assert.ok(hasText(screen, 'target:p2'), 'the atom resolves to its paragraph');
});
