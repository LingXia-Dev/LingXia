import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

const texts = (screen, root) => screen.queryAll(node => node.tag === 'text' && node.props.accessibilityLabel !== undefined);

test('People: a photo is named by alt; without one, the initials are a picture named by the full name; a failed photo falls back', async () => {
  const screen = await renderExample('Avatar/Avatar', 'People');
  assert.equal(screen.queryAll(node => node.tag === 'image' && node.props.imageAlt === 'Ana Lima').length, 1);
  screen.getByText('KC');
  const kai = texts(screen).find(node => node.props.accessibilityLabel === 'Kai Chen');
  assert.ok(kai);
  assert.equal(kai.props.role, 'img');
  const mira = screen.queryAll(node => node.tag === 'image' && node.props.imageAlt === 'Mira Patel')[0];
  screen.image(mira, 'error');
  assert.equal(screen.queryAll(node => node.tag === 'image' && node.props.imageAlt === 'Mira Patel').length, 0);
  assert.ok(texts(screen).some(node => node.props.accessibilityLabel === 'Mira Patel'));
});

test('Group: a named group showing four, the rest as a count with the app\'s words', async () => {
  const screen = await renderExample('Avatar/Avatar', 'Group');
  assert.equal(screen.getByTestId('editors').props.accessibilityLabel, 'Editors');
  const more = screen.getByText('+2');
  assert.equal(more.props.accessibilityLabel, '2 more editors');
});

test('WithName: a decorative avatar is hidden from assistive tech', async () => {
  const screen = await renderExample('Avatar/Avatar', 'WithName');
  assert.equal(screen.queryAll(node => node.props.accessibilityHidden === 1 && node.tag === 'column').length >= 1, true);
});
