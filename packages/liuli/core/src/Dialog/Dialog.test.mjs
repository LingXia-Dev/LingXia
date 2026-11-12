import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const dialogs = screen => screen.queryAll(node => node.tag === 'dialog');

test('Confirm: closed renders nothing; open shows the named panel with title, description and actions', async () => {
  const screen = await renderExample('Dialog/Dialog', 'Confirm');
  assert.equal(dialogs(screen).length, 0);
  screen.press(screen.getByTestId('open'));
  const dialog = screen.getByTestId('dialog');
  assert.equal(dialog.props.accessibilityLabel, 'Delete this note?');
  const heading = screen.getByText('Delete this note?');
  assert.equal(heading.props.role, 'heading');
  assert.equal(textOf(screen.getByText('It leaves every device you are signed in on.')), 'It leaves every device you are signed in on.');
  screen.press(screen.getByTestId('confirm'));
  assert.equal(dialogs(screen).length, 0);
  assert.equal(textOf(screen.getByTestId('outcome')), 'Deleted');
});

test('Form: the close button is named by closeLabel and closes; Save waits for a name', async () => {
  const screen = await renderExample('Dialog/Dialog', 'Form');
  screen.press(screen.getByTestId('open'));
  assert.equal(screen.getByTestId('save').props.onClick, undefined, 'disabled while empty');
  screen.type(screen.getByTestId('name'), 'Recipes');
  screen.press(screen.getByTestId('save'));
  assert.equal(dialogs(screen).length, 0);
  screen.press(screen.getByTestId('open'));
  const close = screen.queryAll(node => node.tag === 'button' && node.props.accessibilityLabel === 'Close');
  assert.equal(close.length, 1);
  screen.press(close[0]);
  assert.equal(dialogs(screen).length, 0);
});
