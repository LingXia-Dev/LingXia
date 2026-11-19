import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const callout = screen => screen.queryAll(node => node.testId === 'tour')[0];
const shownTitle = screen => { const node = callout(screen); return node && node.children.length > 0 ? textOf(node.children[0]) : null; };

test('Welcome: nothing shows until the tour starts; then one callout, anchored to its target and named for it', async () => {
  const screen = await renderExample('Tour/Tour', 'Welcome');
  assert.equal(shownTitle(screen), null);
  screen.press(screen.getByTestId('tour-start'));
  const node = callout(screen);
  assert.deepEqual([shownTitle(screen), node.props.accessibilityLabel], ['Start a note', 'The New note button: Start a note']);
  assert.deepEqual([typeof node.props.anchorTo, node.props.anchorArea], ['string', 'bottom'], 'anchored under its target');
  screen.getByText('1 of 3');
});

test('Welcome: Next and Back step through; the last step says Done and ends the tour; Skip ends it from the first', async () => {
  const screen = await renderExample('Tour/Tour', 'Welcome');
  screen.press(screen.getByTestId('tour-start'));
  screen.press(screen.getByTestId('tour-next'));
  assert.equal(shownTitle(screen), 'Find anything');
  screen.press(screen.getByTestId('tour-back'));
  assert.equal(shownTitle(screen), 'Start a note');
  screen.press(screen.getByTestId('tour-next'));
  screen.press(screen.getByTestId('tour-next'));
  assert.deepEqual([shownTitle(screen), textOf(screen.getByTestId('tour-next'))], ['Share what you choose', 'Done']);
  assert.equal(screen.queryAll(node => node.testId === 'tour-skip').length, 0, 'nothing to skip on the last step');
  screen.press(screen.getByTestId('tour-next'));
  assert.equal(screen.queryAll(node => /^On step/.test(node.text ?? '')).length, 0, 'the tour is not running: no step line');
  screen.press(screen.getByTestId('tour-start'));
  screen.press(screen.getByTestId('tour-skip'));
  assert.equal(screen.queryAll(node => /^On step/.test(node.text ?? '')).length, 0, 'the tour is not running: no step line');
});

test('Welcome: a caret is anchored to the same target by itself, and is not read', async () => {
  const screen = await renderExample('Tour/Tour', 'Welcome');
  screen.press(screen.getByTestId('tour-start'));
  const caret = screen.getByTestId('tour-caret');
  assert.deepEqual([caret.props.anchorArea, caret.props.accessibilityHidden, caret.props.width, caret.children.length], ['bottom', 1, 12, 1]);
  const callout = screen.getByTestId('tour');
  assert.equal(typeof callout.props.anchorTo, 'string', 'the callout hangs from the caret');
});

test('Welcome: the row around the search target wraps, so at large text the field takes its own line instead of shrinking', async () => {
  const screen = await renderExample('Tour/Tour', 'Welcome');
  const search = screen.queryAll(node => node.props.elementId === 'tour-search')[0];
  assert.deepEqual([search.props.grow, search.props.flexBasis, search.props.minWidth], [1, 160, undefined]);
  assert.equal(search.parent.props.flexWrap, 'wrap');
});
