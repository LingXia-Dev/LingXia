import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Goal: a progressbar with the app\'s words, drawn once in the middle and not read twice', async () => {
  const ring = (await renderExample('CircularProgress/CircularProgress', 'Goal')).getByTestId('steps');
  assert.deepEqual([ring.props.role, ring.props.accessibilityLabel, ring.props.ariaValueNow, ring.props.ariaValueMax, ring.props.ariaValueText], ['progressbar', 'Steps today', 7200, 10000, '72%']);
  const words = ring.children[0].children.at(-1);
  assert.deepEqual([words.text, words.props.accessibilityHidden, words.props.opacity, ring.children[0].props.width], ['72%', 1, undefined, 80]);
});

test('At large text: the ring keeps its size and the words move beside it once they outgrow its middle', async () => {
  const screen = await renderExample('CircularProgress/CircularProgress', 'Goal');
  const ring = () => screen.getByTestId('steps');
  const ghost = () => ring().children[0].children.find(node => node.tag === 'text' && node.props.position === 'absolute');
  assert.equal(ghost().text, '72%', 'an unseen copy on one line is measured');
  screen.layout(ghost(), { width: 36, height: 22 });
  assert.equal(ring().children.length, 1, 'fits a 60 pt middle: the words stay inside');
  screen.layout(ghost(), { width: 58, height: 40 });
  const [box, words] = ring().children;
  assert.deepEqual([box.props.width, box.props.height, words.text], [80, 80, '72%']);
  assert.equal(box.children.filter(node => node.tag === 'text').length, 1, 'only the unseen copy stays in the ring');
});

test('InRows: small has no words; without a value it reports none and its arc spins', async () => {
  const screen = await renderExample('CircularProgress/CircularProgress', 'InRows');
  const row = screen.getByTestId('row').children[0];
  assert.deepEqual([row.props.width, row.children.map(node => node.tag)], [24, ['vector', 'vector']]);
  const waiting = screen.getByTestId('waiting');
  assert.equal(waiting.props.ariaValueNow, undefined);
  assert.ok(waiting.children[0].children[1].props.animation, 'the arc spins');
});

test('Tones: a full ring closes, in the tone', async () => {
  const fill = (await renderExample('CircularProgress/CircularProgress', 'Tones')).getByTestId('over').children[0].children[1];
  assert.equal(fill.props.color, 'danger-mark');
});

test('Tones at large text: one ring whose words outgrow its middle sets every ring\'s words beside', async () => {
  const screen = await renderExample('CircularProgress/CircularProgress', 'Tones');
  const rings = () => screen.queryAll(node => node.props.role === 'progressbar');
  const ghosts = () => rings().map(ring => ring.children[0].children.find(node => node.tag === 'text' && node.props.position === 'absolute'));
  const beside = () => rings().map(ring => ring.children.length === 2);
  assert.deepEqual(ghosts().map(node => node.text), ['Done', '86%', 'Over']);
  for (const ghost of ghosts()) screen.layout(ghost, { width: 40, height: 22 });
  assert.deepEqual(beside(), [false, false, false], 'all fit: all inside');
  screen.layout(ghosts()[0], { width: 58, height: 26 });
  assert.deepEqual(beside(), [true, true, true], 'one outgrows its middle: the group moves all beside');
});

test('A lone ring decides by its own words, whatever a group nearby does', async () => {
  const screen = await renderExample('CircularProgress/CircularProgress', 'Goal');
  const ring = () => screen.getByTestId('steps');
  const ghost = ring().children[0].children.find(node => node.tag === 'text' && node.props.position === 'absolute');
  screen.layout(ghost, { width: 36, height: 22 });
  assert.equal(ring().children.length, 1);
});
