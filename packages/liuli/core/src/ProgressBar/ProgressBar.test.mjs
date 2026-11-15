import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

test('Determinate: a named progressbar reporting value, range and the app\'s wording; the fill follows', async () => {
  const screen = await renderExample('ProgressBar/ProgressBar', 'Determinate');
  const bar = () => screen.getByTestId('upload');
  assert.equal(bar().props.role, 'progressbar');
  assert.equal(bar().props.accessibilityLabel, 'Uploading photos');
  assert.deepEqual([bar().props.ariaValueNow, bar().props.ariaValueMin, bar().props.ariaValueMax], [3, 0, 8]);
  assert.equal(bar().props.ariaValueText, '3 of 8');
  assert.equal(bar().children[0].props.widthPercent, 37.5);
  screen.press(screen.getByTestId('more'));
  assert.equal(bar().props.ariaValueNow, 4);
  assert.equal(bar().children[0].props.widthPercent, 50);
  const header = screen.queryAll(node => node.props.accessibilityHidden && textOf(node).includes('Uploading photos'));
  assert.equal(header.length, 1, 'the visible label row is hidden, so the name is read once');
});

test('Indeterminate: no value reported; the fill sweeps', async () => {
  const screen = await renderExample('ProgressBar/ProgressBar', 'Indeterminate');
  const bar = screen.getByTestId('export');
  assert.equal(bar.props.ariaValueNow, undefined);
  assert.equal(bar.props.ariaValueText, undefined);
  assert.match(bar.children[0].props.animation, /"name":"progress-sweep"/);
});

test('Compact: hidden label stays the name; out-of-range values are clamped', async () => {
  const screen = await renderExample('ProgressBar/ProgressBar', 'Compact');
  const bar = screen.getByTestId('storage');
  assert.equal(bar.props.accessibilityLabel, 'Storage used');
  assert.equal(bar.props.height, 4);
  assert.equal(screen.queryAll(node => textOf(node) === '7.2 of 10 GB').length, 0, 'no label row');
  const clamped = await renderPage(`
    import { ProgressBar } from '@liulidesign/core/ProgressBar';
    export function render() { return <page><ProgressBar label="Over" value={140} data-testid="bar" /></page>; }
  `);
  assert.equal(clamped.getByTestId('bar').props.ariaValueNow, 100);
  assert.equal(clamped.getByTestId('bar').children[0].props.widthPercent, 100);
});

test('Spinners: each is a progressbar named by its label, the arc hidden and spinning', async () => {
  const screen = await renderExample('ProgressBar/ProgressBar', 'Spinners');
  const spinner = screen.getByTestId('spinner');
  assert.equal(spinner.props.role, 'progressbar');
  assert.equal(spinner.props.accessibilityLabel, 'Loading messages');
  const arc = spinner.children[0];
  assert.equal(arc.props.accessibilityHidden, 1);
  assert.equal(arc.props.width, 24);
  assert.match(arc.props.animation, /"name":"spin"/);
});

test('Tones: the fill takes the tone, and the words say how it stands', async () => {
  const screen = await renderExample('ProgressBar/ProgressBar', 'Tones');
  const fills = ['tone-goal', 'tone-storage', 'tone-plan'].map(id => screen.getByTestId(id));
  assert.deepEqual(fills.map(bar => bar.children[0].props.background), ['success', 'warning', 'danger']);
  assert.deepEqual(fills.map(bar => bar.props.ariaValueText), ['Reached', '43 of 50 GB, nearly full', 'Over by 1.2 GB']);
});
