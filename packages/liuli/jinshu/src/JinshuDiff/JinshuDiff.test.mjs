import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample, zh } from '../testing/harness.mjs';

test('Revision: the summary counts each kind and the steppers move between changes', async () => {
  const screen = await renderExample('JinshuDiff/JinshuDiff', 'Revision');
  assert.ok(screen.getByText('3 changes'));
  for (const badge of ['+1 added', '~1 changed', '-1 removed']) assert.ok(screen.queryAll(node => node.props.accessibilityLabel === badge || node.text === badge).length > 0, badge);
  assert.ok(screen.getByText('1 / 3'));
});

test('Revision: zh renders the wording it always had', async () => {
  const screen = await renderExample('JinshuDiff/JinshuDiff', 'Revision', zh);
  assert.ok(screen.getByText('共 3 处修改'));
  assert.ok(screen.getByText('上一处'));
});

test('NoChanges: says so, with no steppers', async () => {
  const en = await renderExample('JinshuDiff/JinshuDiff', 'NoChanges');
  assert.ok(en.getByText('No changes'));
  assert.equal(en.queryAll(node => node.text === 'Previous').length, 0);
  const cn = await renderExample('JinshuDiff/JinshuDiff', 'NoChanges', zh);
  assert.ok(cn.getByText('无修改'));
});

test('a block both moved and edited: its changed card carries the same moved badge as a pure move', async () => {
  const { renderPage } = await import('../testing/harness.mjs');
  const screen = await renderPage(
    linked => `import { JinshuDiff } from ${JSON.stringify(`${linked}/src/JinshuDiff/JinshuDiff`)};
import { createDocument, paragraph } from ${JSON.stringify(`${linked}/src/schema/index`)};
const a = createDocument({ blocks: [paragraph('Alpha', 'a'), paragraph('Beta', 'b'), paragraph('Gamma', 'c'), paragraph('Delta', 'd')] });
const b = createDocument({ blocks: [paragraph('Gamma edited', 'c'), paragraph('Delta', 'd'), paragraph('Alpha', 'a'), paragraph('Beta', 'b')] });
export function render() {
  return <page><JinshuDiff a={a} b={b} mode="inline" /></page>;
}
`,
  );
  const badge = label => screen.queryAll(n => n.props.accessibilityLabel === label || n.text === label).length > 0;
  assert.ok(badge('Changed'), 'c is a changed card');
  assert.ok(badge('Moved (#3 → #1)'), 'c also says where it moved');
});
