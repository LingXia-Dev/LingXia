import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

/** What a screen reader reads inside `node`, in order: text and image labels that are not hidden. */
function spoken(node) {
  if (node.props.accessibilityHidden) return [];
  if (node.tag === 'text') return [textOf(node)];
  if (node.props.role === 'img') return [node.props.accessibilityLabel];
  return node.children.flatMap(spoken);
}
const glyph = node => JSON.parse(node.props.vectorSource).paths[0].d;
const find = (node, test) => (test(node) ? [node] : node.children.flatMap(child => find(child, test)));
const delta = stat => find(stat, node => node.tag === 'vector' && node.props.width === 12)[0].parent;

test('Revenue: a group named by its label; the figure, the change in words, the note and the chart are read once each', async () => {
  const screen = await renderExample('Stat/Stat', 'Revenue');
  const stat = screen.getByTestId('revenue');
  assert.equal(stat.props.role, 'group');
  assert.equal(stat.props.accessibilityLabel, 'Revenue');
  assert.deepEqual(spoken(stat), ['$48,250', 'Up 12.4% from last month', 'vs last month', 'Revenue by week: up from 31k to 48k']);
  const value = screen.getByText('$48,250');
  assert.equal(value.props.fontVariantNumeric, 'tabular-nums');
  assert.equal(value.props.fontSize, 28);
  assert.equal(delta(stat).props.color, 'success');
  assert.equal(screen.getByText('12.4%').props.fontVariantNumeric, 'tabular-nums');
});

test('Plain: label and figure only', async () => {
  const screen = await renderExample('Stat/Stat', 'Plain');
  const stat = screen.getByTestId('orders');
  assert.deepEqual(spoken(stat), ['128']);
  assert.equal(stat.children.length, 2);
});

test('Tones: the arrow follows the direction and the colour follows the tone', async () => {
  const screen = await renderExample('Stat/Stat', 'Tones');
  const latency = delta(screen.getByTestId('latency'));
  const errors = delta(screen.getByTestId('errors'));
  assert.equal(latency.props.color, 'success');
  assert.equal(errors.props.color, 'danger');
  assert.notEqual(glyph(latency.children[0]), glyph(errors.children[0]));
  assert.match(glyph(errors.children[0]), /^M6 9\.75v-7\.5/, 'up: the shaft runs from the bottom to the head at the top');
});

test('Sizes: title and display type', async () => {
  const screen = await renderExample('Stat/Stat', 'Sizes');
  const size = id => find(screen.getByTestId(id), node => node.text === '1,204')[0].props.fontSize;
  assert.deepEqual([size('small'), size('medium')], [20, 28]);
});

test('states: by default up is positive, down negative and flat neutral, each with its own arrow', async () => {
  const seen = [];
  for (const [example, id] of [['StateUp', 'up'], ['StateDown', 'down'], ['StateFlat', 'flat']]) {
    const screen = await renderExample('Stat/Stat', example);
    const change = delta(screen.getByTestId(id));
    seen.push([change.props.color, glyph(change.children[0])]);
  }
  assert.deepEqual(seen.map(each => each[0]), ['success', 'danger', 'text-muted']);
  assert.equal(new Set(seen.map(each => each[1])).size, 3);
});

test('StateLoading: busy under the same name, hidden skeletons for the figure and, as a footnote line, the change', async () => {
  const screen = await renderExample('Stat/Stat', 'StateLoading');
  const stat = screen.getByTestId('loading');
  assert.equal(stat.props.stateBusy, 1);
  assert.equal(stat.props.accessibilityLabel, 'Sign-ups');
  assert.deepEqual(spoken(stat), []);
  assert.equal(stat.children.length, 3, 'the label, the figure, and the change line kept');
  assert.match(JSON.stringify(stat.children[1]), /skeleton-glint/);
  const change = stat.children[2].children[0].children[0];
  assert.equal(change.children[0].props.fontSize, 13, 'a footnote line, as the change is');
  assert.ok(!JSON.stringify(stat.children[2]).includes('3.1%'), 'the change itself waits');
});

test('Dashboard: four groups, each read by its label, figure and change', async () => {
  const screen = await renderExample('Stat/Stat', 'Dashboard');
  const groups = screen.queryAll(node => node.props.role === 'group');
  assert.deepEqual(groups.map(group => [group.props.accessibilityLabel, ...spoken(group)]), [
    ['Revenue', '$48,250', 'Up 12.4%'],
    ['Orders', '1,204', 'Up 3.1%'],
    ['Refunds', '37', 'Up 8, worse'],
    ['Avg. order', '$40.07', 'No change'],
  ]);
});
