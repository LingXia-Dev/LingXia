import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const callout = screen => screen.queryAll(node => node.testId === 'tour')[0];
const shownTitle = screen => { const heading = callout(screen)?.children.find(child => child.props.role === 'heading'); return heading ? textOf(heading) : null; };

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

test('Welcome: the callout is anchored to its target with a margin; four carets on its edges, each shown on the side it lands, none read', async () => {
  const screen = await renderExample('Tour/Tour', 'Welcome');
  screen.press(screen.getByTestId('tour-start'));
  const node = callout(screen);
  assert.deepEqual([node.props.marginTop, node.props.marginBottom], [12, 12], 'clear of the target on either side');
  const carets = ['bottom', 'top', 'end', 'start'].map(side => screen.getByTestId(`tour-caret-${side}`));
  assert.ok(carets.every(caret => caret.parent === node && caret.props.accessibilityHidden === 1 && caret.props.position === 'absolute' && caret.props.background === '#00000000' && caret.props.opacity === undefined));
  assert.deepEqual(carets.map((caret, index) => (index < 2 ? caret.props.insetInlineStart : caret.props.insetTop)), ['anchor(center)', 'anchor(center)', 'anchor(center)', 'anchor(center)'], 'each on the target\'s centre line');
  assert.deepEqual(carets.map(caret => Object.keys(caret.props).find(key => /^side.*Style$/.test(key))), ['sideBottomStyle', 'sideTopStyle', 'sideEndStyle', 'sideStartStyle']);
  assert.deepEqual(carets.map(caret => (caret.props.sideBottomStyle ?? caret.props.sideTopStyle ?? caret.props.sideEndStyle ?? caret.props.sideStartStyle)?.background), ['text', 'text', 'text', 'text']);
});

test('Welcome: the row around the search target wraps, so at large text the field takes its own line instead of shrinking', async () => {
  const screen = await renderExample('Tour/Tour', 'Welcome');
  const search = screen.queryAll(node => node.props.elementId === 'tour-search')[0];
  assert.deepEqual([search.props.grow, search.props.flexBasis, search.props.minWidth], [1, 160, undefined]);
  assert.equal(search.parent.props.flexWrap, 'wrap');
});

test('Welcome: at large text the buttons wrap under the step count and stay at the end, each row of them wrapping too', async () => {
  const screen = await renderExample('Tour/Tour', 'Welcome');
  screen.press(screen.getByTestId('tour-start'));
  screen.press(screen.getByTestId('tour-next'));
  const actions = screen.getByTestId('tour-back').parent;
  const footer = actions.parent;
  assert.deepEqual([actions.children.length, footer.children.length], [3, 2], 'Skip, Back and Next beside the count');
  assert.deepEqual([footer.props.flexWrap, actions.props.flexWrap, actions.props.justify], ['wrap', 'wrap', 'end']);
  assert.equal(actions.props.marginInlineStart, 'auto', 'alone on its line, the row keeps to the end');
});
