import { test } from 'node:test';
import assert from 'node:assert/strict';
import { described, renderExample } from '../testing/harness.mjs';

test('Delivery: a named radio group; each option is a radio named by its label and described by its second line', async () => {
  const screen = await renderExample('RadioGroup/RadioGroup', 'Delivery');
  const group = screen.getByTestId('delivery');
  assert.equal(group.props.role, 'radiogroup');
  assert.equal(group.props.accessibilityLabel, 'Delivery');
  const express = screen.getByTestId('delivery-express');
  assert.equal(express.tag, 'button');
  assert.equal(express.props.role, 'radio');
  assert.equal(express.props.accessibilityLabel, 'Express');
  assert.deepEqual(described(screen, express), ['Next working day, €6']);
  screen.press(express);
  assert.equal(screen.getByTestId('delivery-express').props.stateChecked, 1);
  assert.equal(screen.getByTestId('delivery-standard').props.stateChecked, 0);
});

test('Plans: a tap anywhere on a card chooses it', async () => {
  const screen = await renderExample('RadioGroup/RadioGroup', 'Plans');
  screen.press(screen.getByTestId('plan-team'));
  assert.equal(screen.getByTestId('plan-team').props.stateChecked, 1);
});

test('Required: invalid and required until chosen; the error is the group description', async () => {
  const screen = await renderExample('RadioGroup/RadioGroup', 'Required');
  const group = screen.getByTestId('reason');
  assert.equal(group.props.stateInvalid, 1);
  assert.deepEqual(described(screen, group), ['Choose a reason.']);
  screen.press(screen.getByTestId('reason-other'));
  assert.notEqual(screen.getByTestId('reason').props.stateInvalid, 1);
});

test('Inline: the options sit in a row that wraps', async () => {
  const group = (await renderExample('RadioGroup/RadioGroup', 'Inline')).getByTestId('billing');
  assert.deepEqual([group.props.flexDirection, group.props.flexWrap, group.children.length], ['row', 'wrap', 2]);
});

test('At large text: every radio mark keeps its size and its circle, never squeezed by its label', async () => {
  for (const example of ['Plans', 'Delivery', 'Sizes']) {
    const screen = await renderExample('RadioGroup/RadioGroup', example);
    for (const radio of screen.queryAll(node => node.props.role === 'radio')) {
      const { shrink, width, height } = radio.children[0].children[1].props;
      assert.equal(shrink, 0, example);
      assert.ok(Array.isArray(width) && String(width) === String(height), `${example}: a square in em of the label's type`);
    }
  }
});

test('At large text: each radio mark is centred on its label\'s first line, in a line box of the label\'s type', async () => {
  const screen = await renderExample('RadioGroup/RadioGroup', 'Sizes');
  for (const radio of screen.queryAll(node => node.props.role === 'radio')) {
    const [line, text] = radio.children;
    const [ghost, circle] = line.children;
    assert.deepEqual([line.props.flexDirection, line.props.align, line.props.shrink], ['row', 'center', 0]);
    assert.equal(ghost.props.opacity, 0);
    const label = text.children[0];
    assert.deepEqual([ghost.props.fontSize, ghost.props.lineHeight], [label.props.fontSize, label.props.lineHeight]);
    assert.equal(circle.props.marginTop, undefined, 'no fixed nudge: the line centres it');
  }
});
