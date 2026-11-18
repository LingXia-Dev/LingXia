import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Reply: nothing to send, nothing to press; a draft wakes the button, and sending trims it and empties the field', async () => {
  const screen = await renderExample('Composer/Composer', 'Reply');
  const field = () => screen.getByTestId('reply');
  const send = () => screen.getByTestId('reply-send');
  assert.deepEqual([field().props.accessibilityLabel, field().props.rows, field().props.maxRows, send().props.accessibilityLabel, send().props.enabled], ['Message', 1, 5, 'Send', 0]);
  screen.type(field(), '   ');
  assert.equal(send().props.enabled, 0, 'spaces are not a message');
  screen.type(field(), '  Bring the tent  ');
  assert.notEqual(send().props.enabled, 0);
  screen.press(send());
  screen.getByText('Bring the tent');
  assert.equal(field().props.inputValue, '');
});

test('Reply: the keyboard\'s send key sends as the button does', async () => {
  const screen = await renderExample('Composer/Composer', 'Reply');
  screen.type(screen.getByTestId('reply'), 'Leaving now');
  screen.submit(screen.getByTestId('reply'));
  screen.getByText('Leaving now');
});

test('StateSending: the button is busy and takes no second send', async () => {
  const send = (await renderExample('Composer/Composer', 'StateSending')).getByTestId('sending-send');
  assert.equal(send.props.stateBusy, 1);
});

test('Stoppable: while sending the button is Stop and stops; then it sends again', async () => {
  const screen = await renderExample('Composer/Composer', 'Stoppable');
  const button = () => screen.getByTestId('stoppable-send');
  assert.deepEqual([button().props.accessibilityLabel, button().props.stateBusy, button().props.enabled], ['Stop', undefined, undefined]);
  screen.press(button());
  screen.getByText('Stopped');
  assert.equal(button().props.accessibilityLabel, 'Send');
  screen.type(screen.getByTestId('stoppable'), 'And the rest?');
  screen.press(button());
  screen.getByText('Writing a reply…');
});

// The composer's surface, the editor's place, and the measured line, found from the editor.
const partsOf = (screen, id) => {
  const editor = screen.getByTestId(id);
  const field = editor.parent;
  const surface = field.parent;
  const line = surface.children[surface.children.length - 1].children[0];
  return { editor, field, surface, line, send: screen.getByTestId(`${id}-send`) };
};
const isStacked = (screen, id) => partsOf(screen, id).field.props.gridColumnSpan === 3 || /span 3/.test(JSON.stringify(partsOf(screen, id).field.props));

test('Reply: one surface holds the text and the button; the editor has no box of its own', async () => {
  const screen = await renderExample('Composer/Composer', 'Reply');
  const { editor, surface, send } = partsOf(screen, 'reply');
  assert.equal(surface.props.borderWidth ?? surface.props.borderTopWidth, 1, 'the surface draws the edge');
  assert.equal(editor.props.borderWidth ?? 0, 0, 'the editor draws none');
  assert.equal(editor.props.paddingTop ?? 0, 0, 'no inset on the editor: its height is a whole number of lines');
  assert.equal(send.parent, surface, 'the send button is inside the same surface');
});

test('Reply: a draft that fits beside the button stays one row; one that does not is stacked, and a new line always is', async () => {
  const screen = await renderExample('Composer/Composer', 'Reply');
  const sizes = line => {
    const { surface, line: measured } = partsOf(screen, 'reply');
    screen.layout(surface, { width: 320, height: 40 });
    screen.layout(partsOf(screen, 'reply').line, { width: line, height: 22 });
    return measured;
  };
  assert.equal(isStacked(screen, 'reply'), false, 'empty: one row');
  screen.type(partsOf(screen, 'reply').editor, 'Leaving at eight');
  sizes(120);
  assert.equal(isStacked(screen, 'reply'), false, '120 pt of text beside a 40 pt button in 320 pt');
  sizes(260);
  assert.equal(isStacked(screen, 'reply'), true, '260 pt of text does not fit beside the button');
  sizes(120);
  assert.equal(isStacked(screen, 'reply'), false, 'and back when it fits again');
  screen.type(partsOf(screen, 'reply').editor, 'One\nTwo');
  assert.equal(isStacked(screen, 'reply'), true, 'a second line is stacked whatever its width');
});

test('the editor is the same element in both forms, so the caret is kept', async () => {
  const screen = await renderExample('Composer/Composer', 'Reply');
  const before = partsOf(screen, 'reply').surface.children.map(child => child.tag);
  screen.type(partsOf(screen, 'reply').editor, 'One\nTwo');
  assert.deepEqual(partsOf(screen, 'reply').surface.children.map(child => child.tag), before, 'the same children in the same order');
});

test('Attached: what is attached sits inside the surface over the text, and can be removed', async () => {
  const screen = await renderExample('Composer/Composer', 'Attached');
  const { surface } = partsOf(screen, 'attached');
  assert.equal(isStacked(screen, 'attached'), true);
  assert.ok(JSON.stringify(surface.children[0]).includes('Route.gpx'), 'the first row of the surface');
  screen.press(screen.queryAll(node => node.props.accessibilityLabel === 'Remove Route.gpx')[0]);
  assert.equal(screen.queryAll(node => node.text === 'Route.gpx').length, 0);
  screen.getByText('Packing list.pdf');
});
