import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample } from '../testing/harness.mjs';

const dialogs = screen => screen.queryAll(node => node.tag === 'dialog');
// The scrim fades in from nothing and back out to it on the panel's own timing.
const presence = node => { const { durationMs, easing } = JSON.parse(node.props.transition); return { durationMs, easing, enters: node.props.startingStyle !== undefined, leaves: node.props.exitStyle !== undefined }; };
const timing = node => { const { durationMs, easing } = JSON.parse(node.props.transition); return { durationMs, easing, enters: true, leaves: true }; };

test('Confirm: the scrim arrives and leaves with the dialog, on its timing', async () => {
  const screen = await renderExample('Dialog/Dialog', 'Confirm');
  screen.press(screen.getByTestId('confirm-open'));
  assert.deepEqual(presence(screen.getByTestId('confirm-dialog-backdrop')), timing(screen.getByTestId('confirm-dialog')));
});

test('Confirm: closed renders nothing; open, an alert dialog named by its title and described by its text', async () => {
  const screen = await renderExample('Dialog/Dialog', 'Confirm');
  assert.equal(dialogs(screen).length, 0);
  screen.press(screen.getByTestId('confirm-open'));
  const dialog = screen.getByTestId('confirm-dialog');
  assert.equal(dialog.props.accessibilityLabel, 'Delete this note?');
  assert.equal(dialog.props.role, 'alertdialog');
  assert.deepEqual(described(screen, dialog), ['It leaves every device you are signed in on.']);
  const heading = screen.getByText('Delete this note?');
  assert.equal(heading.props.role, 'heading');
  assert.equal(textOf(screen.getByText('It leaves every device you are signed in on.')), 'It leaves every device you are signed in on.');
  screen.press(screen.getByTestId('confirm'));
  assert.equal(dialogs(screen).length, 0);
  assert.equal(textOf(screen.getByTestId('outcome')), 'Deleted');
});

test('Form: the close button is named "Close" by default and closes; Save waits for a name', async () => {
  const screen = await renderExample('Dialog/Dialog', 'Form');
  screen.press(screen.getByTestId('rename-open'));
  assert.equal(screen.getByTestId('rename-dialog').props.role, 'dialog');
  assert.equal(screen.getByTestId('rename-dialog').props.ariaDescribedBy, undefined);
  assert.equal(screen.getByTestId('save').props.onClick, undefined, 'disabled while empty');
  screen.type(screen.getByTestId('name'), 'Recipes');
  screen.press(screen.getByTestId('save'));
  assert.equal(dialogs(screen).length, 0);
  screen.press(screen.getByTestId('rename-open'));
  const close = screen.queryAll(node => node.tag === 'button' && node.props.accessibilityLabel === 'Close');
  assert.equal(close.length, 1);
  screen.press(close[0]);
  assert.equal(dialogs(screen).length, 0);
});

test('Full: covers the screen with square corners and no floating shadow; the body takes the room left', async () => {
  const screen = await renderExample('Dialog/Dialog', 'Full');
  screen.press(screen.getByTestId('full-open'));
  const dialog = screen.getByTestId('full-dialog');
  assert.deepEqual([dialog.props.widthPercent, dialog.props.heightPercent, dialog.props.radius, dialog.props.boxShadow, dialog.props.background], [100, 100, 0, undefined, 'canvas']);
  assert.equal(dialog.children.find(node => node.tag === 'scroll').props.grow, 1);
});

test('Hero: the picture is drawn only, and the close button over it still closes', async () => {
  const screen = await renderExample('Dialog/Dialog', 'Hero');
  screen.press(screen.getByTestId('hero-open'));
  const dialog = screen.getByTestId('hero-dialog');
  assert.equal(dialog.props.accessibilityLabel, 'Shared notebooks');
  const [media] = dialog.children;
  assert.ok(media.children[0].props.accessibilityHidden, 'the picture is not read');
  const close = media.children[1];
  assert.equal(close.props.accessibilityLabel, 'Close');
  assert.equal(close.props.background, 'media');
  screen.press(close);
  assert.equal(screen.queryAll(node => node.props.testId === 'hero-dialog').length, 0);
});

test('Parts and the tile: the drawn panel\'s secondary action takes the overlay\'s surface, as in a dialog', async () => {
  for (const [name, label] of [['Parts', 'Cancel'], ['Specimen', 'Keep']]) {
    const screen = await renderExample('Dialog/Dialog', name);
    const button = screen.queryAll(node => node.tag === 'button' && textOf(node) === label)[0];
    assert.equal(button.props.background, 'surface-overlay', name);
  }
});
