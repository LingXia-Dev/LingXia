import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Documents: the button is named by the field and its words; the files are a list with their state in words', async () => {
  const screen = await renderExample('FileInput/FileInput', 'Documents');
  const pick = screen.getByTestId('documents');
  assert.deepEqual([pick.tag, pick.props.accessibilityLabel], ['button', 'Documents: Choose files']);
  const list = screen.getByTestId('documents-files');
  assert.deepEqual([list.props.role, list.props.accessibilityLabel, list.children.length], ['list', 'Documents', 3]);
  screen.getByText('Too large: 25 MB at most');
  const ring = screen.getByTestId('documents-meter').children.find(node => node.props.role === 'progressbar');
  assert.deepEqual([ring.props.accessibilityLabel, ring.props.ariaValueNow], ['Uploading Meter reading, November.jpg', 64]);
  assert.equal(screen.getByTestId('documents-scan').props.borderColor, 'danger');
});

test('Documents: the app adds what its picker returns, and a remove button takes a file out', async () => {
  const screen = await renderExample('FileInput/FileInput', 'Documents');
  screen.press(screen.getByTestId('documents'));
  assert.equal(screen.getByTestId('documents-files').children.length, 4);
  const remove = screen.getByTestId('documents-scan-remove');
  assert.equal(remove.props.accessibilityLabel, 'Remove Passport scan.tiff');
  screen.press(remove);
  assert.equal(screen.queryAll(node => node.testId === 'documents-scan').length, 0);
});

test('Empty: no list, and the error colours the button', async () => {
  const screen = await renderExample('FileInput/FileInput', 'Empty');
  assert.equal(screen.queryAll(node => node.props.role === 'list').length, 0);
  assert.equal(screen.getByTestId('proof').props.borderColor, 'danger');
});
