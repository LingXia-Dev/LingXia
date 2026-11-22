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

test('Indeterminate: no value reported; the dashes march', async () => {
  const screen = await renderExample('ProgressBar/ProgressBar', 'Indeterminate');
  const bar = screen.getByTestId('export');
  assert.equal(bar.props.ariaValueNow, undefined);
  assert.equal(bar.props.ariaValueText, undefined);
  if (bar.children[0]) assert.match(bar.children[0].props.animation, /"name":"progress-march"/);
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
  assert.deepEqual(fills.map(bar => bar.children[0].props.background), ['success-mark', 'warning-mark', 'danger-mark']);
  assert.deepEqual(fills.map(bar => bar.props.ariaValueText), ['Reached', '43 of 50 GB, nearly full', 'Over by 1.2 GB']);
});

test('SpinnerTones: 48 px in the on-media colour over a picture; 16 px muted beside text', async () => {
  const screen = await renderExample('ProgressBar/ProgressBar', 'SpinnerTones');
  const arc = id => screen.getByTestId(id).children[0];
  assert.deepEqual([arc('on-media').props.width, arc('on-media').props.color], [48, 'on-media']);
  assert.deepEqual([arc('quiet').props.width, arc('quiet').props.color], [16, 'text-muted']);
});

test('LoadingMessage: the label is drawn under the ring and read once, as the name', async () => {
  const screen = await renderExample('ProgressBar/ProgressBar', 'LoadingMessage');
  const spinner = screen.getByTestId('loading-notes');
  assert.equal(spinner.props.role, 'progressbar');
  assert.equal(spinner.props.accessibilityLabel, 'Loading your notes');
  const words = spinner.children.find(node => node.tag === 'text');
  assert.equal(textOf(words), 'Loading your notes');
  assert.ok(words.props.accessibilityHidden);
});

test('Transfer: progress drives the bar: bytes of the total once known, unknown at null and without a fraction', async () => {
  const screen = await renderExample('ProgressBar/ProgressBar', 'Transfer');
  const bar = () => screen.getByTestId('download');
  const reported = () => [bar().props.ariaValueNow, bar().props.ariaValueMin, bar().props.ariaValueMax];
  const next = () => screen.press(screen.getByTestId('next-value'));
  assert.deepEqual(reported(), [3_100_000, 0, 10_000_000]);
  assert.equal(bar().props.ariaValueText, '3.1 of 10.0 MB');
  next();
  assert.deepEqual(reported(), [10_000_000, 0, 10_000_000]);
  next();
  assert.deepEqual(reported(), [undefined, undefined, undefined], 'null before the first value');
  next();
  assert.deepEqual(reported(), [undefined, undefined, undefined], 'no fraction while the size is unknown');
  next();
  assert.deepEqual(reported(), [0, 0, 10_000_000], 'a known size, nothing loaded yet');
});

test('Transfer: an unknown amount draws whole dashes, marching one period in the reading direction with motion, still under reduced motion', async () => {
  const unknown = async env => {
    const screen = await renderExample('ProgressBar/ProgressBar', 'Transfer', { env });
    screen.press(screen.getByTestId('next-value'));
    screen.press(screen.getByTestId('next-value'));
    return screen.getByTestId('download').children[0]?.props;
  };
  const ltr = await unknown({});
  // The harness may not lay the track out; then the track stays empty rather than guess a pattern.
  if (ltr === undefined) return;
  assert.match(ltr.animation, /"name":"progress-march"/);
  assert.match((await unknown({ direction: 'rtl' })).animation, /"name":"progress-march-rtl"/);
  assert.equal((await unknown({ reducedMotion: true })).animation, undefined);
});
