import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const texts = (screen, role) => screen.queryAll(node => node.props.role === role && node.tag === 'text').map(textOf);
// What the live texts of a role say now, ignoring the cleared ones.
const live = (screen, role) => texts(screen, role).filter(text => text !== '');
const shown = screen => screen.queryAll(node => node.testId === 'toast');
const message = screen => textOf(shown(screen)[0].children.find(node => node.tag === 'text'));

test('Archive: a toast shows and is announced; Undo runs and closes it', async () => {
  const screen = await renderExample('Toast/Toast', 'Archive');
  assert.equal(shown(screen).length, 0);
  assert.deepEqual(live(screen, 'status'), []);
  screen.press(screen.getByTestId('archive-q3-plan'));
  assert.equal(shown(screen).length, 1);
  assert.equal(message(screen), '"Q3 plan" archived');
  assert.deepEqual(live(screen, 'status'), ['"Q3 plan" archived']);
  assert.deepEqual(live(screen, 'alert'), []);
  screen.press(screen.getByTestId('toast-action'));
  assert.equal(shown(screen).length, 0);
  screen.getByTestId('archive-q3-plan');
});

test('Archive: the dismiss button is named and closes the toast', async () => {
  const screen = await renderExample('Toast/Toast', 'Archive');
  screen.press(screen.getByTestId('archive-reading-list'));
  const dismiss = screen.getByTestId('toast-dismiss');
  assert.equal(dismiss.props.accessibilityLabel, 'Dismiss');
  screen.press(dismiss);
  assert.equal(shown(screen).length, 0);
  assert.equal(screen.queryAll(node => node.testId === 'archive-reading-list').length, 0);
});

test('Archive: a swipe to either side closes it; a drag back to rest keeps it', async () => {
  const screen = await renderExample('Toast/Toast', 'Archive');
  screen.press(screen.getByTestId('archive-q3-plan'));
  const card = () => screen.queryAll(node => node.props.drag !== undefined)[0];
  assert.equal(card().props.accessibilityLabel, '"Q3 plan" archived');
  screen.drag(card(), { phase: 'start' });
  screen.drag(card(), { phase: 'end', index: 1, velocity: 0 });
  assert.equal(shown(screen).length, 1);
  screen.drag(card(), { phase: 'end', index: 2, velocity: 900 });
  assert.equal(shown(screen).length, 0);
});

test('Tones: toasts queue and show one at a time; a danger toast is an alert', async () => {
  const screen = await renderExample('Toast/Toast', 'Tones');
  screen.press(screen.getByTestId('show-success'));
  screen.press(screen.getByTestId('show-danger'));
  assert.equal(shown(screen).length, 1);
  assert.equal(message(screen), 'Changes saved');
  assert.deepEqual(live(screen, 'status'), ['Changes saved']);
  screen.press(screen.getByTestId('toast-dismiss'));
  assert.equal(message(screen), 'Could not save the note');
  assert.deepEqual(live(screen, 'status'), []);
  assert.deepEqual(live(screen, 'alert'), ['Could not save the note']);
  screen.press(screen.getByTestId('toast-dismiss'));
  assert.equal(shown(screen).length, 0);
});

test('Tones: the same message twice in a row is announced twice, by a live text that was cleared first', async () => {
  const screen = await renderExample('Toast/Toast', 'Tones');
  screen.press(screen.getByTestId('show-success'));
  screen.press(screen.getByTestId('show-success'));
  const first = texts(screen, 'status');
  assert.deepEqual(first.filter(text => text !== ''), ['Changes saved']);
  screen.press(screen.getByTestId('toast-dismiss'));
  const second = texts(screen, 'status');
  assert.equal(message(screen), 'Changes saved');
  const speaking = second.findIndex(text => text === 'Changes saved');
  assert.ok(speaking >= 0);
  assert.equal(first[speaking], '', 'the live text now speaking was empty in the frame before, so its text changed');
});

test('Progress: a toast with the same id updates in place instead of queueing', async () => {
  const screen = await renderExample('Toast/Toast', 'Progress');
  screen.press(screen.getByTestId('upload'));
  screen.press(screen.getByTestId('upload'));
  assert.equal(shown(screen).length, 1);
  assert.equal(message(screen), 'Uploaded 2 of 3');
  screen.press(screen.getByTestId('upload'));
  assert.equal(message(screen), 'All 3 photos uploaded');
  screen.press(screen.getByTestId('toast-dismiss'));
  assert.equal(shown(screen).length, 0);
});

test('a touch screen centres the toast; a pointer puts it at the bottom end', async () => {
  for (const [pointer, align] of [['coarse', 'center'], ['fine', 'end']]) {
    const screen = await renderExample('Toast/Toast', 'Tones', { env: { pointer } });
    screen.press(screen.getByTestId('show-info'));
    const region = screen.queryAll(node => node.props.elementId !== undefined && node.props.justify === 'end')[0];
    assert.equal(region.props.align, align);
    assert.equal(region.props.paddingBottom, 16);
  }
});

test('the toast clears the bars and the home indicator, and sits on an open keyboard', async () => {
  const insets = { top: 92, right: 0, bottom: 34, left: 0 };
  const phone = await renderExample('Toast/Toast', 'Tones', { env: { insets } });
  phone.press(phone.getByTestId('show-info'));
  assert.equal(region(phone).props.paddingBottom, 50);
  assert.equal(region(phone).props.paddingTop, 108);
  const typing = await renderExample('Toast/Toast', 'Tones', { env: { insets, keyboard: { height: 300 } } });
  typing.press(typing.getByTestId('show-info'));
  assert.equal(region(typing).props.paddingBottom, 16);
});

const region = screen => screen.queryAll(node => node.props.elementId !== undefined && node.props.justify === 'end')[0];
// The top-layer box the popover lands in carries its wire fields.
const layer = screen => screen.queryAll(node => node.props.topLayer === 2)[0];

test('a toast without an action closes after the region\'s duration; the next one shows', async () => {
  const screen = await renderExample('Toast/Toast', 'Tones');
  screen.press(screen.getByTestId('show-success'));
  screen.press(screen.getByTestId('show-info'));
  assert.equal(layer(screen).props.closeAfter, 5000);
  screen.host.__lingxiaCloseAfter(region(screen).props.elementId);
  screen.rerender();
  assert.equal(message(screen), 'Link copied');
  screen.host.__lingxiaCloseAfter(region(screen).props.elementId);
  screen.rerender();
  assert.equal(shown(screen).length, 0);
});

test('a toast with an action never closes by itself', async () => {
  const screen = await renderExample('Toast/Toast', 'Archive');
  screen.press(screen.getByTestId('archive-q3-plan'));
  assert.equal(layer(screen).props.closeAfter, undefined);
});

test('a toast\'s own duration overrides the region\'s', async () => {
  const screen = await renderExample('Toast/Toast', 'Tones');
  screen.press(screen.getByTestId('show-warning'));
  assert.equal(layer(screen).props.closeAfter, 8000);
});
