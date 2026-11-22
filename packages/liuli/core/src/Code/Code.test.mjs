import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Command: a named group with its title, the app\'s copy action, and the code unwrapped in a sideways scroll', async () => {
  const screen = await renderExample('Code/Code', 'Command');
  const block = screen.getByTestId('install');
  assert.deepEqual([block.props.role, block.props.accessibilityLabel], ['group', 'Install commands']);
  const [header, scroll] = block.children;
  assert.equal(header.children[0].text, 'Terminal');
  const code = scroll.children[0];
  assert.deepEqual([scroll.tag, code.props.whiteSpace, code.text.split('\n').length], ['scroll', 'pre', 2]);
  screen.press(screen.getByTestId('copy'));
  screen.getByText('Copied');
});

test('Plain: no header without a title or an action', async () => {
  const block = (await renderExample('Code/Code', 'Plain')).getByTestId('response');
  assert.deepEqual(block.children.map(node => node.tag), ['scroll']);
});

test('InSentence: code is a run of its sentence, on a tint', async () => {
  const screen = await renderExample('Code/Code', 'InSentence');
  // A run has no node of its own, so no test id: it is found by its words.
  const [run] = screen.queryAll(node => node.tag === 'span' && node.text === 'minRuntime');
  assert.deepEqual([run.tag, run.props.background], ['span', 'text/8']);
});

test('Numbered: numbers from the start line beside one selectable text, marked lines tinted behind it', async () => {
  const block = (await renderExample('Code/Code', 'Numbered')).getByTestId('numbered');
  const lines = block.children[1].children[0];
  const [tints, gutter, code] = lines.children;
  assert.equal(gutter.props.accessibilityHidden, 1, 'numbers are not read');
  assert.equal(gutter.children.map(span => span.text.trim()).join(','), '41,42,43,44,45');
  assert.equal(code.children.map(span => span.text).join(''), 'export function total(items: readonly Item[]) {\n  let sum = 0;\n  for (const item of items) sum += item.price * item.count;\n  return Math.round(sum * 100) / 100;\n}');
  assert.equal(tints.children.length, 5);
  assert.deepEqual(tints.children.map(row => row.props.background ?? null), [null, null, 'accent-soft', 'accent-soft', null]);
});

test('Diff: signs in their own column, the code without them, headers quiet', async () => {
  const block = (await renderExample('Code/Code', 'Diff')).getByTestId('diff');
  const [tints, signs, code] = block.children[1].children[0].children;
  assert.equal(signs.children.map(span => span.text.replace('\n', '')).join(''), '     -+ ');
  assert.equal(code.children[3].text, '  let sum = 0;\n');
  assert.equal(code.children[6].text, '    sum += item.price * item.count;\n');
  assert.deepEqual(tints.children.map(row => row.props.background ?? null), ['text/6', 'text/6', 'text/6', null, null, 'danger-soft', 'success-soft', null]);
});

test('diffKinds: a file header only before a hunk; a lone "---" line is a removal', async () => {
  const { diffKinds } = await import('./diff.ts');
  assert.deepEqual(diffKinds(['--- a', '+++ b', '@@ -1 +1 @@', '-x', '+y', ' z']), ['header', 'header', 'header', 'removed', 'added', 'context']);
  assert.deepEqual(diffKinds(['--- not a header', 'y']), ['removed', 'context']);
});
