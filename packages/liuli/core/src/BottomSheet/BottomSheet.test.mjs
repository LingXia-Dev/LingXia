import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const sheet = (screen, testId) => screen.queryAll(node => node.testId === testId)[0];
const drag = node => JSON.parse(node.props.drag);
// The scrim fades in from nothing and back out to it on the panel's own timing.
const presence = node => { const { durationMs, easing } = JSON.parse(node.props.transition); return { durationMs, easing, enters: node.props.startingStyle !== undefined, leaves: node.props.exitStyle !== undefined }; };
const timing = node => { const { durationMs, easing } = JSON.parse(node.props.transition); return { durationMs, easing, enters: true, leaves: true }; };

test('MoveTo: the scrim arrives and leaves with the sheet, on its timing', async () => {
  const screen = await renderExample('BottomSheet/BottomSheet', 'MoveTo');
  screen.press(screen.getByTestId('move-open'));
  assert.deepEqual(presence(sheet(screen, 'move-backdrop')), timing(sheet(screen, 'move')));
});

test('MoveTo: a named dialog on the bottom edge, as tall as its content, that drags off', async () => {
  const screen = await renderExample('BottomSheet/BottomSheet', 'MoveTo');
  assert.equal(sheet(screen, 'move'), undefined);
  screen.press(screen.getByTestId('move-open'));
  const panel = sheet(screen, 'move');
  assert.equal(panel.props.accessibilityLabel, 'Move to');
  assert.deepEqual(drag(panel), { axis: 'y', points: [0, '100%'], index: 0 });
  screen.press(screen.getByTestId('folder-Travel'));
  assert.equal(sheet(screen, 'move'), undefined);
  assert.equal(textOf(screen.getByTestId('folder')), 'In Travel');
});

test('MoveTo: coming to rest off screen closes it, and the next open starts at the top again', async () => {
  const screen = await renderExample('BottomSheet/BottomSheet', 'MoveTo');
  screen.press(screen.getByTestId('move-open'));
  screen.drag(sheet(screen, 'move'), { phase: 'settle', index: 1, hidden: true });
  assert.equal(sheet(screen, 'move'), undefined);
  screen.press(screen.getByTestId('move-open'));
  assert.equal(drag(sheet(screen, 'move')).index, 0);
});

test('Comments: an expandable sheet opens halfway, drags up, and reopens halfway after the close button', async () => {
  const screen = await renderExample('BottomSheet/BottomSheet', 'Comments');
  screen.press(screen.getByTestId('comments-open'));
  assert.deepEqual(drag(sheet(screen, 'comments')), { axis: 'y', points: [0, '45%', '100%'], index: 1 });
  screen.drag(sheet(screen, 'comments'), { phase: 'settle', index: 0 });
  assert.equal(drag(sheet(screen, 'comments')).index, 0);
  const close = screen.getByTestId('comments-close');
  assert.equal(close.props.accessibilityLabel, 'Close');
  screen.press(close);
  assert.equal(sheet(screen, 'comments'), undefined);
  screen.press(screen.getByTestId('comments-open'));
  assert.equal(drag(sheet(screen, 'comments')).index, 1);
});

test('Required: with dismissOn none the sheet has no off-screen point and only its buttons close it', async () => {
  const screen = await renderExample('BottomSheet/BottomSheet', 'Required');
  screen.press(screen.getByTestId('leave-open'));
  assert.deepEqual(drag(sheet(screen, 'leave')).points, [0]);
  screen.press(screen.getByTestId('stay'));
  assert.equal(sheet(screen, 'leave'), undefined);
  assert.equal(textOf(screen.getByTestId('answer')), 'Stayed');
});

test('the panel clears the home indicator by the safe area', async () => {
  const screen = await renderExample('BottomSheet/BottomSheet', 'MoveTo', { env: { safeArea: { top: 0, right: 0, bottom: 34, left: 0 } } });
  screen.press(screen.getByTestId('move-open'));
  assert.equal(sheet(screen, 'move').props.paddingBottom, 54);
});
