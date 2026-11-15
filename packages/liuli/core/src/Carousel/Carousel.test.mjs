import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Photos: a named list of full-width pages that the scroll stops on, with on-media dots over it', async () => {
  const screen = await renderExample('Carousel/Carousel', 'Photos');
  const [scroll, dots] = screen.getByTestId('photos').children;
  const track = scroll.children[0];
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
