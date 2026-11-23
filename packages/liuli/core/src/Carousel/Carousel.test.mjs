import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Photos: a named list of full-width pages that the scroll stops on, with on-media dots over it', async () => {
  const screen = await renderExample('Carousel/Carousel', 'Photos');
  const [frame, dots] = screen.getByTestId('photos').children;
  const [scroll] = frame.children;
  const track = scroll.children[0];
  assert.equal(frame.children.length, 1, 'no buttons unless asked');
  assert.deepEqual([scroll.tag, track.props.role, track.props.accessibilityLabel, track.children.length], ['scroll', 'list', 'Photos of the house', 4]);
  assert.ok(track.children.every(page => page.props.role === 'listitem' && page.props.widthPercent === 100 && page.props.shrink === 0));
  assert.deepEqual([dots.props.position, dots.children[0].props.accessibilityLabel, dots.children[0].children[0].props.background], ['absolute', 'Photo 1 of 4', 'on-media']);
});

test('Welcome: a dot and the app\'s button move the same page', async () => {
  const screen = await renderExample('Carousel/Carousel', 'Welcome');
  const widths = () => screen.getByTestId('welcome-dots').children.map(dot => dot.children[0].props.width);
  screen.press(screen.getByTestId('next'));
  assert.deepEqual(widths(), [6, 18, 6]);
  screen.press(screen.getByTestId('welcome-dots-2'));
  assert.deepEqual(widths(), [6, 6, 18]);
  screen.getByText('Get started');
});

test('Looping: buttons over both edges; past the last photo comes the first, and before the first the last', async () => {
  const screen = await renderExample('Carousel/Carousel', 'Looping');
  const current = () => screen.getByTestId('looping-dots').children.findIndex(dot => dot.children[0].props.width === 18);
  assert.equal(screen.getByTestId('looping-previous').props.accessibilityLabel, 'Previous photo');
  screen.press(screen.getByTestId('looping-previous'));
  assert.equal(current(), 3);
  screen.press(screen.getByTestId('looping-next'));
  assert.equal(current(), 0);
});

test('Rail: cards of the given width with a gap, stopped on one by one; no previous button at the start', async () => {
  const screen = await renderExample('Carousel/Carousel', 'Rail');
  const frame = screen.getByTestId('rail').children[0];
  const track = frame.children[0].children[0];
  assert.deepEqual([track.children.length, track.children[0].props.width, track.props.gap], [5, 160, 12]);
  assert.equal(screen.queryAll(node => node.testId === 'rail-previous').length, 0);
  screen.press(screen.getByTestId('rail-next'));
  assert.equal(screen.getByTestId('rail-previous').props.accessibilityLabel, 'Previous albums');
  assert.equal(screen.queryAll(node => node.testId === 'rail-dots').length, 0);
});

test('Welcome: a page held by the caller drives the snap, and a dot reports its page back', async () => {
  const screen = await renderExample('Carousel/Carousel', 'Welcome');
  const scroller = () => screen.queryAll(node => node.tag === 'scroll')[0];
  assert.equal(scroller().props.snapIndex, 0);
  screen.press(screen.getByTestId('next'));
  assert.equal(scroller().props.snapIndex, 1, 'the caller\'s index reaches the scroll');
  const dots = screen.getByTestId('welcome-dots');
  screen.press(screen.queryAll(node => node.tag === 'button' && node.parent?.parent === dots || node.tag === 'button' && node.parent === dots)[2]);
  assert.equal(scroller().props.snapIndex, 2, 'a dot goes through onIndexChange');
});
