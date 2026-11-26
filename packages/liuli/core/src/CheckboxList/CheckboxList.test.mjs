import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample } from '../testing/harness.mjs';

const box = (screen, id) => screen.getByTestId(id);
const isOn = node => node.props.checked === 1 || node.props.dataState === 'checked' || JSON.stringify(node.props).includes('"checked"');

test('Notifications: a group named by its question; each option is its own checkbox; the value is the chosen list in the options\' order', async () => {
  const screen = await renderExample('CheckboxList/CheckboxList', 'Notifications');
  const group = box(screen, 'topics');
  assert.equal(group.props.role, 'group');
  assert.equal(group.props.accessibilityLabel, 'Notify me about');
  const question = group.children[0];
  assert.deepEqual([textOf(question), question.props.accessibilityHidden], ['Notify me about', 1], 'the question is read once, as the group\'s name');
  const mentions = box(screen, 'topics-mentions');
  assert.equal(mentions.tag, 'checkbox');
  assert.equal(mentions.props.accessibilityLabel, 'Mentions');
  assert.deepEqual(described(screen, mentions), ['When someone names you in a note.']);
  assert.equal(textOf(box(screen, 'chosen')), 'mentions');
  screen.toggle(box(screen, 'topics-digest'), true);
  screen.toggle(box(screen, 'topics-comments'), true);
  assert.equal(textOf(box(screen, 'chosen')), 'mentions, comments, digest', 'in the options\' order, not the order chosen');
  screen.toggle(box(screen, 'topics-mentions'), false);
  assert.equal(textOf(box(screen, 'chosen')), 'comments, digest');
});

test('AtLeastOne: the group\'s help text and its status are its description; the status goes once the choice stands', async () => {
  const screen = await renderExample('CheckboxList/CheckboxList', 'AtLeastOne');
  const group = () => box(screen, 'days');
  assert.deepEqual(described(screen, group()), ['We book a desk for each day you choose.', 'Choose at least one day.']);
  assert.equal(textOf(group().children[0]), 'Office days *');
  screen.toggle(box(screen, 'days-wed'), true);
  assert.deepEqual(described(screen, group()), ['We book a desk for each day you choose.']);
});

test('WithUnavailable and StateDisabled: one option, or all of them, take no press', async () => {
  const screen = await renderExample('CheckboxList/CheckboxList', 'WithUnavailable');
  assert.equal(box(screen, 'toppings-anchovies').props.enabled, 0);
  assert.notEqual(box(screen, 'toppings-olives').props.enabled, 0);
  assert.deepEqual(described(screen, box(screen, 'toppings-anchovies')), ['Sold out today.']);
  const disabled = await renderExample('CheckboxList/CheckboxList', 'StateDisabled');
  assert.deepEqual(['disabled-mentions', 'disabled-comments'].map(id => box(disabled, id).props.enabled), [0, 0]);
});

test('Sizes: the options take the list\'s size', async () => {
  const screen = await renderExample('CheckboxList/CheckboxList', 'Sizes');
  // Each size's box in `em` of its label's type: 16 and 24 pt at the default type.
  const boxSize = id => { const { width, fontSize } = box(screen, id).children[0].children[0].children[1].props; return Math.round(width[1] * fontSize); };
  assert.deepEqual([boxSize('small-a'), boxSize('large-a')], [16, 24]);
});

test('SelectAll: a dash while some are chosen; a press chooses all, the next clears all', async () => {
  const screen = await renderExample('CheckboxList/CheckboxList', 'SelectAll');
  const all = () => screen.getByTestId('export-all');
  const state = () => JSON.parse(all().props.dataAttributes).state;
  assert.equal(state(), 'mixed');
  screen.toggle(all(), true);
  assert.equal(state(), 'checked');
  assert.equal(JSON.parse(screen.getByTestId('export-garden').props.dataAttributes).state, 'checked');
  screen.toggle(all(), false);
  assert.equal(state(), 'unchecked');
});

test('WithEndContent: an option\'s end content shares its label\'s line, never gives way, and is read after the label', async () => {
  const screen = await renderExample('CheckboxList/CheckboxList', 'WithEndContent');
  const notes = box(screen, 'backup-notes');
  assert.equal(notes.props.accessibilityLabel, 'Notes');
  assert.deepEqual(described(screen, notes), ['18 MB']);
  const size = screen.queryAll(node => node.tag === 'text' && node.text === '18 MB')[0];
  const end = size.parent;
  assert.equal(end.props.shrink, 0, 'the size never gives way');
  assert.equal(end.parent.children[0].text, 'Notes', 'it shares the label\'s line');
  assert.equal(end.parent.props.flexDirection, 'row');
});
