import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample, renderPage } from '../testing/harness.mjs';

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
  assert.equal(card('yearly').props.accessibilityLabel, 'Yearly');
  assert.deepEqual(described(screen, card('yearly')), ['€40 a year']);
  assert.equal(card('monthly').props.stateChecked, 1);
  assert.equal(card('yearly').props.stateChecked, 0);
  assert.equal(card('monthly').props.stateSelected, undefined);
  screen.press(card('yearly'));
  assert.equal(card('monthly').props.stateChecked, 0);
  assert.equal(card('yearly').props.stateChecked, 1);
  assert.equal(card('yearly').props.borderColor, 'accent');
});

test('ClickableCard: alone, a chosen card is reported as pressed, not selected or checked', async () => {
  const screen = await renderExample('Card/Card', 'StateSelected');
  const [card] = screen.queryAll(node => node.tag === 'button');
  assert.equal(card.props.statePressed, 1);
  assert.equal(card.props.stateSelected, undefined);
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

test('Floating: the raised surface under the raised shadow, with a fainter edge', async () => {
  const card = (await renderExample('Card/Card', 'Floating')).getByTestId('floating');
  assert.deepEqual([card.props.background, card.props.borderColor], ['surface-raised', 'line']);
  assert.equal(JSON.parse(card.props.boxShadow)[1].blur, 24);
});

test('ToPage: the card is one block link, and the Like button inside it keeps its own press', async () => {
  const screen = await renderExample('Card/Card', 'ToPage');
  const card = screen.getByTestId('to-page');
  assert.equal(card.tag, 'link-box');
  assert.equal(card.props.accessibilityLabel, 'Spring launch, 12 tasks');
  assert.equal(screen.getByTestId('to-page-like').tag, 'button');
  screen.press(screen.getByTestId('to-page-like'));
  assert.equal(screen.getByTestId('to-page-like').props.statePressed, 1, 'pressed, and drawn so');
  assert.equal(screen.getByTestId('to-page').props.accessibilityLabel, 'Spring launch, 12 tasks', 'the card stayed');
});

test('Choice: a card is named by its plan and described by its price', async () => {
  const screen = await renderExample('Card/Card', 'Choice');
  const card = screen.queryAll(node => node.testId?.startsWith('plan-'))[0];
  assert.equal(described(screen, card).length, 1);
  assert.notEqual(described(screen, card)[0], '');
  assert.ok(!card.props.accessibilityLabel.includes(described(screen, card)[0]), 'the price is the description, not part of the name');
});

test('Several: a group of pressed cards, each on and off on its own, with a check that follows', async () => {
  const screen = await renderExample('Card/Card', 'Several');
  assert.equal(screen.getByTestId('topics').props.role, 'group');
  const card = id => screen.getByTestId(`topic-${id}`);
  const checked = id => JSON.stringify(card(id).children).includes('"vector"');
  assert.deepEqual(['design', 'code', 'research'].map(id => card(id).props.statePressed), [1, 0, 0]);
  assert.deepEqual([card('design').props.role, card('design').props.stateChecked], [undefined, undefined]);
  assert.deepEqual([checked('design'), checked('code')], [true, false]);
  screen.press(card('code'));
  screen.press(card('research'));
  screen.press(card('design'));
  assert.deepEqual(['design', 'code', 'research'].map(id => card(id).props.statePressed), [0, 1, 1]);
  assert.equal(textOf(screen.getByTestId('topics-count')), '2 topics');
  assert.equal(card('code').props.accessibilityLabel, 'Code');
  assert.equal(card('code').props.paddingInlineEnd, 40, 'content keeps 8 pt clear of the check');
});
