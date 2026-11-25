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
