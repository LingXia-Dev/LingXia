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
    assert.equal(ghost.text, '\u200B', 'a zero-width run: nothing drawn, no layer');
    assert.equal(ghost.props.opacity, undefined);
    const label = text.children[0];
    assert.deepEqual([ghost.props.fontSize, ghost.props.lineHeight], [label.props.fontSize, label.props.lineHeight]);
    assert.equal(circle.props.marginTop, undefined, 'no fixed nudge: the line centres it');
  }
});

test('StateDisabled: the chosen circle is a neutral fill with a muted dot, not a faded accent', async () => {
  const screen = await renderExample('RadioGroup/RadioGroup', 'StateDisabled');
  const chosen = screen.queryAll(node => node.props.role === 'radio' && node.props.stateChecked)[0] ?? screen.queryAll(node => node.props.role === 'radio')[0];
  const circle = chosen.children[0].children[1];
  assert.deepEqual([circle.props.background, circle.children[0].props.background], ['text/20', 'text-muted']);
  assert.equal(chosen.props.opacity, undefined, 'the row is not faded');
});

test('Plans: a price at the end of the name\'s line, read after the name, before the second line', async () => {
  const screen = await renderExample('RadioGroup/RadioGroup', 'Plans');
  const plus = screen.getByTestId('plan-plus');
  assert.equal(plus.props.accessibilityLabel, 'Plus');
  assert.deepEqual(described(screen, plus), ['€4 a month', 'Unlimited notebooks, sync across devices']);
  const price = screen.queryAll(node => node.tag === 'text' && node.text === '€4 a month')[0];
  const end = price.parent;
  assert.equal(end.props.shrink, 0, 'the price never gives way');
  assert.equal(end.parent.children[0].text, 'Plus', 'it shares the label\'s line');
  assert.equal(end.parent.props.flexDirection, 'row');
});

test('props are described by what they are, not by one product\'s use', async () => {
  const { readFile } = await import('node:fs/promises');
  const source = await readFile(new URL('./RadioGroup.tsx', import.meta.url), 'utf8');
  assert.match(source, /\/\*\* Trailing content at the end of the label's line: a price, a size, a count\. [^\n]*\*\/\n\s*endContent\?/);
  const catalog = JSON.parse(await readFile(new URL('../../catalog.json', import.meta.url), 'utf8'));
  const radio = (catalog.components ?? catalog).find(part => part.name === 'RadioGroup');
  const variant = radio.props.find(prop => prop.name === 'variant');
  assert.doesNotMatch(variant.description, /plan/i);
});
