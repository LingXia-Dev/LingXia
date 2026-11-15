import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Goal: a progressbar with the app\'s words, drawn once in the middle and not read twice', async () => {
  const ring = (await renderExample('CircularProgress/CircularProgress', 'Goal')).getByTestId('steps');
  assert.deepEqual([ring.props.role, ring.props.accessibilityLabel, ring.props.ariaValueNow, ring.props.ariaValueMax, ring.props.ariaValueText], ['progressbar', 'Steps today', 7200, 10000, '72%']);
  const words = ring.children.at(-1);
  assert.deepEqual([words.text, words.props.accessibilityHidden, ring.props.width], ['72%', 1, 80]);
});

test('InRows: small has no words; without a value it reports none and its arc spins', async () => {
  const screen = await renderExample('CircularProgress/CircularProgress', 'InRows');
  const row = screen.getByTestId('row');
  assert.deepEqual([row.props.width, row.children.map(node => node.tag)], [24, ['vector', 'vector']]);
  const waiting = screen.getByTestId('waiting');
  assert.equal(waiting.props.ariaValueNow, undefined);
  assert.ok(waiting.children[1].props.animation, 'the arc spins');
});

test('Tones: a full ring closes, in the tone', async () => {
  const fill = (await renderExample('CircularProgress/CircularProgress', 'Tones')).getByTestId('over').children[1];
  assert.equal(fill.props.color, 'danger-mark');
});
