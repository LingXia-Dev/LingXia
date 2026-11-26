import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

const dialogs = screen => screen.queryAll(node => node.tag === 'dialog');
// The scrim fades in from nothing and back out to it on the panel's own timing.
const presence = node => { const { durationMs, easing } = JSON.parse(node.props.transition); return { durationMs, easing, enters: node.props.startingStyle !== undefined, leaves: node.props.exitStyle !== undefined }; };
const timing = node => { const { durationMs, easing } = JSON.parse(node.props.transition); return { durationMs, easing, enters: true, leaves: true }; };

test('Filters: the scrim arrives and leaves with the drawer, on its timing', async () => {
  const screen = await renderExample('Drawer/Drawer', 'Filters');
  screen.press(screen.getByTestId('filters-open'));
  assert.deepEqual(presence(screen.getByTestId('filters-backdrop')), timing(screen.getByTestId('filters')));
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

test('isLabelHidden keeps the label as the name without a title, the close button at the end, marked <id>-close', async () => {
  const { renderPage } = await import('../testing/harness.mjs');
  const { textOf } = await import('@lingxia/view/testing');
  const screen = await renderPage(`
    import { Drawer } from '@liulidesign/core/Drawer';
    export function render() { return <page><Drawer isOpen onOpenChange={() => {}} label="Filters" isLabelHidden data-testid="filters"><text>Body</text></Drawer></page>; }`);
  const panel = screen.getByTestId('filters');
  assert.equal(panel.props.accessibilityLabel, 'Filters');
  assert.equal(screen.queryAll(node => node.props.role === 'heading').length, 0);
  assert.ok(!textOf(panel).includes('Filters'));
  const close = screen.getByTestId('filters-close');
  assert.equal(close.parent.props.justify, 'end');
});
