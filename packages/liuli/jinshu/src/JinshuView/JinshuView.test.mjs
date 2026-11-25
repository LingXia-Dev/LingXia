import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, zh } from '../testing/harness.mjs';

const labelOf = node => node.props.accessibilityLabel;

test('Lesson: the title is a level-one heading and each block keeps its id', async () => {
  const screen = await renderExample('JinshuView/JinshuView', 'Lesson');
  assert.equal(textOf(screen.getByText('Limits')), 'Limits');
  for (const id of ['h1', 'p1', 'l1', 'c1', 'k1', 'f1']) assert.ok(screen.getByTestId(`jinshu-block-${id}`), id);
});

test('Lesson: the file and code chrome speak the page language', async () => {
  const en = await renderExample('JinshuView/JinshuView', 'Lesson');
  assert.ok(en.getByText('12 pages · 470.7 KB'));
  assert.ok(en.getByText('Download'));
  const cn = await renderExample('JinshuView/JinshuView', 'Lesson', zh);
  assert.ok(cn.getByText('12 页 · 470.7 KB'));
  assert.ok(cn.getByText('下载'));
});

test('Resume: the bar names the heading and offers to continue', async () => {
  const en = await renderExample('JinshuView/JinshuView', 'Resume');
  assert.ok(en.getByText('You stopped at “What a limit says”'));
  assert.ok(en.queryAll(node => labelOf(node) === 'Start from the beginning').length > 0);
  const cn = await renderExample('JinshuView/JinshuView', 'Resume', zh);
  assert.ok(cn.getByText('上次读到「What a limit says」'));
  assert.ok(cn.getByText('继续阅读'));
});

test('Unavailable: one line in place of the document', async () => {
  const en = await renderExample('JinshuView/JinshuView', 'Unavailable');
  assert.ok(en.getByText("This document isn't available"));
  const cn = await renderExample('JinshuView/JinshuView', 'Unavailable', zh);
  assert.ok(cn.getByText('文档暂不可用'));
});

test('openAt: the block where a change landed scrolls into view on its first layout', async () => {
  const { renderPage } = await import('../testing/harness.mjs');
  const screen = await renderPage(
    linked => `import { JinshuView } from ${JSON.stringify(`${linked}/src/JinshuView/JinshuView`)};
import { createDocument, paragraph } from ${JSON.stringify(`${linked}/src/schema/index`)};
const doc = createDocument({ title: 'T', blocks: [paragraph('One.', 'a'), paragraph('Two.', 'b')] });
export function render() {
  return <page><JinshuView doc={doc} openAt="b" /></page>;
}
`,
  );
  const target = screen.queryAll(n => String(n.props.elementId ?? n.props.id ?? '').endsWith('-o-b'));
  assert.equal(target.length, 1);
  assert.ok(target[0].props.onLayout, 'reveals itself once laid out');
  assert.equal(screen.queryAll(n => String(n.props.elementId ?? n.props.id ?? '').endsWith('-o-a')).length, 0);
});

test('One document twice on a page: each instance scopes its element ids', async () => {
  const { renderPage } = await import('../testing/harness.mjs');
  const screen = await renderPage(
    linked => `import { useLocal } from '@lingxia/view';
import { JinshuView } from ${JSON.stringify(`${linked}/src/JinshuView/JinshuView`)};
import { JinshuEditor } from ${JSON.stringify(`${linked}/src/JinshuEditor/JinshuEditor`)};
import { JinshuDiff } from ${JSON.stringify(`${linked}/src/JinshuDiff/JinshuDiff`)};
import { useJinshuNav } from ${JSON.stringify(`${linked}/src/JinshuOutline/JinshuOutline`)};
import { createDocument, heading, paragraph, table } from ${JSON.stringify(`${linked}/src/schema/index`)};
const a = createDocument({ title: 'T', blocks: [heading(2, 'H', 'h'), paragraph('One.', 'p'), table(['A'], [['1']], { id: 't' })] });
const b = createDocument({ title: 'T', blocks: [heading(2, 'H', 'h'), paragraph('Two.', 'p')] });
export function render() {
  const nav = useJinshuNav('lesson-nav');
  return <page>
    <JinshuView doc={a} nav={nav} />
    <JinshuView doc={a} />
    <JinshuEditor doc={a} onOps={() => {}} hasLeaveGuard={false} />
    <JinshuEditor doc={a} onOps={() => {}} hasLeaveGuard={false} />
    <JinshuDiff a={a} b={b} />
    <JinshuDiff a={a} b={b} />
  </page>;
}
`,
  );
  const ids = screen.queryAll(n => typeof (n.props.elementId ?? n.props.id) === 'string').map(n => n.props.elementId ?? n.props.id);
  assert.deepEqual(ids.filter((id, i) => ids.indexOf(id) !== i), []);
  // The nav's scope names the blocks it scrolls to.
  assert.equal(ids.filter(id => id === 'jinshu-lesson-nav-b-p').length, 1);
});
