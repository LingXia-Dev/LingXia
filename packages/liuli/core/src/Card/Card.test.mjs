import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample, renderPage } from '../testing/harness.mjs';

test('Variants: a plain view with the container radius, not a control', async () => {
  const screen = await renderExample('Card/Card', 'Variants');
  const card = screen.getByTestId('outlined');
  assert.notEqual(card.tag, 'button');
  assert.equal(card.props.accessibilityLabel, undefined);
  assert.equal(card.props.radius, 14);
  assert.equal(card.props.background, 'surface');
  assert.equal(card.props.paddingTop, 16);
});

test('WithActions: the buttons inside a plain card stay their own controls', async () => {
  const screen = await renderExample('Card/Card', 'WithActions');
  assert.equal(screen.getByTestId('turn-on').props.accessibilityLabel, 'Turn on');
});

test('Choice: the cards are a radio group named by its label; pressing one checks it', async () => {
  const screen = await renderExample('Card/Card', 'Choice');
  const card = id => screen.getByTestId(`plan-${id}`);
  const group = screen.getByTestId('plans');
  assert.equal(group.props.role, 'radiogroup');
  assert.equal(group.props.accessibilityLabel, 'Plan');
  assert.equal(card('monthly').tag, 'button');
  assert.equal(card('monthly').props.role, 'radio');
  assert.equal(card('yearly').props.accessibilityLabel, 'Yearly, €40 a year');
  assert.equal(card('monthly').props.stateChecked, 1);
  assert.equal(card('yearly').props.stateChecked, 0);
  assert.equal(card('monthly').props.stateSelected, undefined);
  screen.press(card('yearly'));
  assert.equal(card('monthly').props.stateChecked, 0);
  assert.equal(card('yearly').props.stateChecked, 1);
  assert.equal(card('yearly').props.borderColor, 'accent');
});

test('ClickableCard: alone, a chosen card is reported as selected, not checked', async () => {
  const screen = await renderExample('Card/Card', 'StateSelected');
  const [card] = screen.queryAll(node => node.tag === 'button');
  assert.equal(card.props.stateSelected, 1);
  assert.equal(card.props.role, undefined);
  assert.equal(card.props.stateChecked, undefined);
});

test('ClickableCard: a control inside is refused by the View', async () => {
  await assert.rejects(renderPage(`
      import { ClickableCard } from '@liulidesign/core/Card';
      export function render() {
        return <page><ClickableCard label="Plan" onClick={() => {}}><button onClick={() => {}}>Inner</button></ClickableCard></page>;
      }
  `), /UI_BUTTON/);
});

test('a pressed card keeps its surface under a wash of the text colour', async () => {
  const screen = await renderExample('Card/Card', 'Choice');
  const card = screen.queryAll(node => node.tag === 'button' && node.props.pressedStyle)[0];
  assert.equal(card.props.pressedStyle.background, undefined, 'the fill is not swapped for another surface');
  assert.deepEqual(JSON.parse(card.props.pressedStyle.ring).inner, { width: 999, color: 'text/7' });
});
