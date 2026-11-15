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

test('Presence: the status is a dot that is not read; its words follow the name; offline is hollow', async () => {
  const screen = await renderExample('Avatar/Avatar', 'Presence');
  const dot = id => screen.getByTestId(id).children.find(node => node.props.position === 'absolute' && node.tag !== 'image' && node.props.accessibilityHidden === 1);
  assert.deepEqual(['online', 'away', 'busy'].map(id => dot(id).props.background), ['success', 'warning', 'danger']);
  assert.deepEqual([dot('offline').props.background, Boolean(dot('offline').props.ring)], ['surface', true]);
  assert.deepEqual([dot('online').props.width, dot('online').props.borderColor], [12, 'surface'], 'cut out of the avatar by the surface colour');
  const named = id => screen.getByTestId(id).children.map(node => node.props.accessibilityLabel ?? node.props.alt).filter(Boolean);
  assert.deepEqual(named('away'), ['Kai Chen, Away']);
  assert.ok(screen.getByTestId('online').children.some(node => Object.values(node.props).includes('Ana Lima, Online')), 'a photo is named the same way');
});

test('Unread: a badge sits on the top end edge and is read by its own label', async () => {
  const screen = await renderExample('Avatar/Avatar', 'Unread');
  const slot = screen.getByTestId('unread').children.at(-1);
  assert.equal(slot.props.position, 'absolute');
  assert.equal(slot.props.accessibilityHidden, undefined);
  screen.getByText('3');
});
