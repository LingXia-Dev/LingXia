import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Welcome: a tab per page named by the app; pressing one goes there and stretches its mark', async () => {
  const screen = await renderExample('PageControl/PageControl', 'Welcome');
  const dots = () => screen.getByTestId('welcome').children;
  assert.deepEqual(dots().map(dot => [dot.props.role, dot.props.accessibilityLabel]), [['tab', 'Step 1 of 3'], ['tab', 'Step 2 of 3'], ['tab', 'Step 3 of 3']]);
  assert.deepEqual(dots().map(dot => dot.children[0].props.width), [18, 6, 6]);
  screen.press(screen.getByTestId('welcome-2'));
  assert.deepEqual(dots().map(dot => dot.children[0].props.width), [6, 6, 18]);
  screen.getByText('Share what you choose');
});

test('OnMedia: the marks take the on-media colour', async () => {
  const dots = (await renderExample('PageControl/PageControl', 'OnMedia')).getByTestId('photos').children;
  assert.deepEqual(dots.map(dot => dot.children[0].props.background), ['on-media/40', 'on-media', 'on-media/40', 'on-media/40', 'on-media/40']);
});
