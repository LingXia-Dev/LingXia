import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Card',
  category: 'content',
  status: 'preview',
  since: '2026-11-12',
  summary: 'A surface that groups related content, or one pressable choice',
  description:
    'Card groups a heading, text and actions that belong together. ClickableCard, from the same import, makes the whole ' +
    'card one button, for opening an item or choosing one of a few options; it is named by a short `label` and must not ' +
    'contain other controls. ClickableCardGroup makes cards with a `value` one radio group.',
  keywords: ['card', 'panel', 'tile', 'surface', 'container', 'box', 'clickable card', 'option card', 'choice'],
  propsType: 'CardProps',
  otherPropsTypes: ['ClickableCardProps', 'ClickableCardGroupProps'],
  recipe: { map: 'cardRecipe', parts: 'cardParts' },
  anatomy: [
    { part: 'root', required: true, description: 'A `view` for Card, a `button` for ClickableCard; fill, border, radius and inset.' },
    { part: 'group', required: false, description: 'ClickableCardGroup\'s radio group: one Tab stop, named by its label.' },
  ],
  accessibility: {
    pattern: 'button',
    name: 'label',
    description: ['description (ClickableCard)'],
    states: [
      { state: 'selected', from: 'isSelected (ClickableCard)' },
      { state: 'checked', from: 'the group\'s value or bind (ClickableCard in a ClickableCardGroup)' },
      { state: 'disabled', from: 'isDisabled (ClickableCard)' },
    ],
    keyboard: [
      { keys: 'Up / Left, Down / Right', effect: 'In a ClickableCardGroup: the previous or next card, chosen on arrival.' },
    ],
    contrast: [
      { what: 'selected border', pair: ['accent', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'Card is not a control; its content is read in order. The button pattern applies to ClickableCard.',
      'ClickableCard is read as its `label` only, so the label summarises what the card shows.',
      'Selection is a thicker accent border as well as a colour, so it does not rest on colour alone.',
      'In a ClickableCardGroup each card is a radio: checked, not selected, and the group takes one Tab stop.',
    ],
  },
  guidance: {
    do: [
      'Use a plain Card with Buttons inside when a card has actions.',
      'Give a ClickableCard a label that a listener can choose by: "Yearly, 40 a year".',
      'Put cards that choose one value in a ClickableCardGroup, not side by side with isSelected.',
      'Keep the card variant the same for cards that sit side by side.',
    ],
    dont: [
      'Put a Button, a menu or an input inside a ClickableCard; the View refuses a control inside a button.',
      'Nest a raised card inside another raised card.',
      'Rely on the outline alone to show where a card ends; it is decorative, below 3:1.',
    ],
  },
  examples: [
    { name: 'Variants', title: 'Outlined, raised and sunken' },
    { name: 'Floating', title: 'Floating over a map' },
    { name: 'WithActions', title: 'A card with actions' },
    { name: 'Choice', title: 'Choosing one card of several' },
    { name: 'ToPage', title: 'A card that goes to a page' },
  ],
  gaps: [
    {
      missing: 'A focus ring that stays distinct on a selected card.',
      needs: 'ring offset: a gap between the box and its ring',
      today: 'The selected border and the focus ring sit in the same place.',
    },
  ],
  playground: { props: {  }, fixture: true },
  related: ['Item', 'Button'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Related content',
    order: 1,
    page: {
      stage: 'bare',
      lead: 'Use a card to group content that belongs together, and a clickable card when the whole card opens or chooses something.',
      hero: 'WithActions',
      variants: [
        { name: 'Variants', title: 'Outlined, raised and sunken' },
        { name: 'Floating', title: 'Floating, over a map or a picture' },
        { name: 'ToPage', title: 'Going to a page, with a Like button of its own' },
      ],
      states: [
        { name: 'StateUnselected', title: 'Clickable' },
        { name: 'StateSelected', title: 'Selected' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
      inContext: [{ name: 'Choice', title: 'Choosing a plan' }],
    },
  },
};
