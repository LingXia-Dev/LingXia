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

test('Choice: each card is one button named by its label; pressing one moves the selection', async () => {
  const screen = await renderExample('Card/Card', 'Choice');
  const card = id => screen.getByTestId(`plan-${id}`);
  assert.equal(card('monthly').tag, 'button');
  assert.equal(card('yearly').props.accessibilityLabel, 'Yearly, €40 a year');
  assert.equal(card('monthly').props.stateSelected, 1);
  assert.equal(card('yearly').props.stateSelected, undefined);
  screen.press(card('yearly'));
  assert.equal(card('monthly').props.stateSelected, undefined);
  assert.equal(card('yearly').props.stateSelected, 1);
  assert.equal(card('yearly').props.borderColor, 'accent');
});

test('ClickableCard: a control inside is refused by the View', async () => {
  await assert.rejects(renderPage(`
      import { ClickableCard } from '@liulidesign/core/Card';
      export function render() {
        return <page><ClickableCard label="Plan" onClick={() => {}}><button onClick={() => {}}>Inner</button></ClickableCard></page>;
      }
  `), /UI_BUTTON/);
});
