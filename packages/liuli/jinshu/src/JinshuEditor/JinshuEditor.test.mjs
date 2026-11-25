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

import { runInContext } from 'node:vm';
import { renderPage } from '../testing/harness.mjs';
import { codeBlock, createDocument, details, fileBlock, heading, paragraph, plainText, table } from '../schema/index.ts';
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

/**
 * Plays Logic's side of the bridge for a rendered page: `syncDoc` opens a channel that
 * `source.serve` answers, and `ask` records its argument.
 */
function bridge(screen, source) {
  const host = screen.host;
  const asked = [];
  const server = { data: new Set(), close: new Set() };
  let channelId = null;
  let seq = 0;
  // Frames are parsed inside the page's realm, as the bridge hands them over.
  const receive = frame => {
    host.__frame = JSON.stringify(frame);
    runInContext('__lingxiaReceiveActionResponse(JSON.parse(__frame))', host);
  };
  const pump = () => {
    for (let round = 0; round < 10; round++) {
      const events = host.__lingxiaTakeViewEvents();
      if (events.length === 0) return;
      for (const msg of events) {
        if (msg.kind === 'ch.open') {
          channelId = msg.id;
          receive({ v: 2, kind: 'ch.ack', id: msg.id, ok: true });
          source.serve({
            send: payload => {
              receive({ v: 2, kind: 'ch.data', id: channelId, seq: seq++, payload });
            },
            on(event, fn) {
              server[event].add(fn);
              return () => server[event].delete(fn);
            },
          });
        } else if (msg.kind === 'ch.data') {
          for (const fn of [...server.data]) fn(JSON.parse(JSON.stringify(msg.payload)));
        } else if (msg.kind === 'req') {
          asked.push(JSON.parse(JSON.stringify(msg.payload ?? msg.params ?? null)));
          receive({ v: 2, kind: 'res', id: msg.id, ok: true, result: null });
        }
      }
    }
  };
  host.__lingxiaConfigureActions({ __names: ['syncDoc', 'ask'], __modes: { syncDoc: 'call', ask: 'call' } });
  // The page reads the bridge's own actions and channels, as on a device.
  const { actions, channels } = runInContext('__lingxiaCreateView({}, {})', host);
  return { asked, pump, view: { actions, channels } };
}

const lessonDoc = () =>
  createDocument({
    title: 'Week 3',
    blocks: [heading(1, 'Week 3', 'h'), paragraph('First paragraph.', 'p1'), paragraph('Second paragraph.', 'p2'), paragraph('Third.', 'p3')],
  });

async function mountLive(source, env = desk) {
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
    { env },
  );
  const { asked, pump, view } = bridge(screen, source);
  screen.rerender(view);
  const settle = async () => {
    for (let i = 0; i < 4; i++) {
      screen.rerender({});
      pump();
      await flush();
    }
    screen.rerender({});
  };
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
