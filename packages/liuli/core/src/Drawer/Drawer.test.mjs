import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

const dialogs = screen => screen.queryAll(node => node.tag === 'dialog');
const timing = node => { const { durationMs, easing } = JSON.parse(node.props.transition); return { durationMs, easing }; };

test('Filters: the scrim keeps the drawer\'s timing, so the two leave together', async () => {
  const screen = await renderExample('Drawer/Drawer', 'Filters');
  screen.press(screen.getByTestId('filters-open'));
  assert.deepEqual(timing(screen.getByTestId('filters-backdrop')), timing(screen.getByTestId('filters')));
});

test('Filters: closed renders nothing; open, a dialog named by its title on the end edge, the height of the screen', async () => {
  const screen = await renderExample('Drawer/Drawer', 'Filters');
  assert.equal(dialogs(screen).length, 0);
  screen.press(screen.getByTestId('filters-open'));
  const drawer = screen.getByTestId('filters');
  assert.deepEqual([drawer.props.accessibilityLabel, drawer.props.heightPercent, drawer.props.maxWidth, drawer.props.marginInlineEnd], ['Filters', 100, 420, 0]);
  assert.equal(drawer.children.find(node => node.tag === 'scroll').props.grow, 1, 'the body takes the height left');
  screen.press(screen.getByTestId('filters-apply'));
  assert.equal(dialogs(screen).length, 0);
});

test('Navigation: from the start edge, narrow, described by the line under its title', async () => {
  const screen = await renderExample('Drawer/Drawer', 'Navigation');
  screen.press(screen.getByTestId('nav-open'));
  const drawer = screen.getByTestId('nav');
  assert.deepEqual([drawer.props.maxWidth, drawer.props.marginInlineStart], [320, 0]);
  assert.ok(drawer.props.ariaDescribedBy);
  screen.press(screen.getByTestId('nav-all'));
  assert.equal(dialogs(screen).length, 0);
});
