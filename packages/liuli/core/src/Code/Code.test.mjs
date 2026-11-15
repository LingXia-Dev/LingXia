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
  const run = (await renderExample('Code/Code', 'InSentence')).getByTestId('key');
  assert.deepEqual([run.tag, run.props.background], ['span', 'text/8']);
});
