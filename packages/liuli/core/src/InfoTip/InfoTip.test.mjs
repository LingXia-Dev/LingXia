import { test } from 'node:test';
import assert from 'node:assert/strict';
import { described, renderExample } from '../testing/harness.mjs';

test('Storage: a named info button; a press opens a tooltip note that then describes the button', async () => {
  const screen = await renderExample('InfoTip/InfoTip', 'Storage');
  const trigger = screen.getByTestId('storage');
  assert.equal(trigger.tag, 'button');
  assert.equal(trigger.props.accessibilityLabel, 'About storage');
  // Expanded comes from the popover's own state, not a prop.
  assert.equal(trigger.props.stateExpanded, 0);
  assert.equal(screen.queryAll(node => node.testId === 'storage-content').length, 0);
  screen.press(trigger);
  const note = screen.getByTestId('storage-content');
  assert.equal(note.props.role, 'tooltip');
  assert.equal(screen.getByTestId('storage').props.stateExpanded, 1);
  assert.deepEqual(described(screen, screen.getByTestId('storage')), ['Attachments count toward storage. Notes themselves do not.']);
});
