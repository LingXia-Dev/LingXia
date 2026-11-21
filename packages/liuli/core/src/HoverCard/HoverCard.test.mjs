import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample, renderPage } from '../testing/harness.mjs';

test('Person: words as the trigger are wrapped in one button that shows a hint card on interest and on a press', async () => {
  const screen = await renderExample('HoverCard/HoverCard', 'Person');
  const trigger = screen.getByTestId('ana');
  assert.equal(trigger.tag, 'button');
  assert.equal(trigger.children[0].tag, 'text', 'the words stay the author\'s own');
  assert.equal(screen.queryAll(node => node.tag === 'button').length, 1, 'one focus stop');
  assert.ok(trigger.props.interestFor, 'shown while a pointer or focus rests on it');
  assert.ok(trigger.props.onClick, 'and by a press');
  screen.press(trigger);
  const card = screen.getByTestId('ana-card');
  assert.equal(card.props.accessibilityLabel, 'About Ana Lima');
  assert.equal(card.props.role, 'dialog', 'named, so a dialog');
  assert.equal(card.props.modal, undefined);
});

test('Term: the card opens at the side of its trigger', async () => {
  const screen = await renderExample('HoverCard/HoverCard', 'Term');
  screen.press(screen.getByTestId('seat'));
  const card = screen.getByTestId('seat-card');
  assert.ok(JSON.stringify(card.props).includes('inline-end'));
});

test('a Link mid-sentence shows the card on interest and on a press', async () => {
  const screen = await renderPage(`
    import { HoverCard } from '@liulidesign/core/HoverCard';
    import { Link } from '@liulidesign/core/Link';
    export function render() { return <page><text>Due <HoverCard data-testid="due" content={<text>Sunday 18:00</text>}><Link label="Sunday" data-testid="when" /></HoverCard>, at six.</text></page>; }
  `);
  const link = screen.getByTestId('when');
  assert.equal(link.tag, 'link');
  assert.ok(link.props.interestFor);
  screen.press(link);
  assert.equal(screen.getByTestId('when').props.stateExpanded, 1);
});

test('InSentence: the link stays in its sentence and wires the overlay', async () => {
  const screen = await renderExample('HoverCard/HoverCard', 'InSentence');
  const link = screen.queryAll(node => node.tag === 'link')[0];
  assert.ok(link, 'a link');
  assert.equal(screen.queryAll(node => node.tag === 'text' && node.children?.includes(link)).length, 1, 'inside the sentence');
  assert.ok(link.props.interestFor || link.props.stateExpanded === 0, 'wired to its overlay');
});

test('Term: the words that show the card are underlined, dotted, by the trigger itself', async () => {
  const screen = await renderExample('HoverCard/HoverCard', 'Term');
  const { textDecorationLine, textDecorationStyle } = screen.getByTestId('seat').props;
  assert.deepEqual([textDecorationLine, textDecorationStyle], ['underline', 'dotted']);
});
