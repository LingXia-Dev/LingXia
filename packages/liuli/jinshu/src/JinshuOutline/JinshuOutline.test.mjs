import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, zh } from '../testing/harness.mjs';

test('Progress: a group named by its title, one button per heading', async () => {
  const screen = await renderExample('JinshuOutline/JinshuOutline', 'Progress');
  const outline = screen.getByTestId('outline');
  assert.equal(outline.props.role, 'group');
  assert.equal(outline.props.accessibilityLabel, 'Contents');
  assert.equal(screen.queryAll(node => node.tag === 'button').length, 4);
  assert.ok(screen.getByText('(Untitled)'));
});

test('Progress: the default title follows the page language', async () => {
  const screen = await renderExample('JinshuOutline/JinshuOutline', 'Progress', zh);
  assert.equal(screen.getByTestId('outline').props.accessibilityLabel, '目录');
});

test('Progress: pressing a heading marks it the one being read', async () => {
  const screen = await renderExample('JinshuOutline/JinshuOutline', 'Progress');
  screen.press(screen.getByText('Exercises').parent);
  assert.equal(screen.getByText('Exercises').props.color, 'accent');
});

test('Progress: a status is said in words and its glyph is hidden', async () => {
  const label = node => node.props.accessibilityLabel;
  const en = await renderExample('JinshuOutline/JinshuOutline', 'Progress');
  for (const name of ['Limits, done', 'Definition, in progress', 'Exercises, locked']) assert.equal(en.queryAll(node => label(node) === name).length, 1, name);
  for (const glyph of ['✓', '●', '○']) assert.equal(en.queryAll(node => node.tag === 'text' && textOf(node) === glyph)[0].props.accessibilityHidden, 1, glyph);
  const cn = await renderExample('JinshuOutline/JinshuOutline', 'Progress', zh);
  assert.equal(cn.queryAll(node => label(node) === 'Exercises，未解锁').length, 1);
});
