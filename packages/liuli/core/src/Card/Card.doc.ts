import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Card',
  category: 'container',
  status: 'lab',
  summary: 'A surface that groups related content, or one pressable choice',
  description:
    'Card groups a heading, text and actions that belong together. ClickableCard, from the same import, makes the whole ' +
    'card one button, for opening an item or choosing one of a few options; it is named by a short `label` and must not ' +
    'contain other controls.',
  keywords: ['card', 'panel', 'tile', 'surface', 'container', 'box', 'clickable card', 'option card', 'choice'],
  propsType: 'CardProps',
  otherPropsTypes: ['ClickableCardProps'],
  recipe: { map: 'cardRecipe', parts: 'cardParts' },
  anatomy: [
    { part: 'root', required: true, description: 'A `view` for Card, a `button` for ClickableCard; fill, border, radius and inset.' },
  ],
  accessibility: {
    pattern: 'button',
    name: 'label',
    description: [],
    states: [
      { state: 'selected', from: 'isSelected (ClickableCard)' },
      { state: 'disabled', from: 'isDisabled (ClickableCard)' },
    ],
    keyboard: [],
    contrast: [
      { what: 'selected border', pair: ['accent', 'surface'], minimum: '3:1', proven: true },
    ],
    notes: [
      'Card is not a control; its content is read in order. The button pattern applies to ClickableCard.',
      'ClickableCard is read as its `label` only, so the label summarises what the card shows.',
      'Selection is a thicker accent border as well as a colour, so it does not rest on colour alone.',
    ],
  },
  guidance: {
    do: [
      'Use a plain Card with Buttons inside when a card has actions.',
      'Give a ClickableCard a label that a listener can choose by: "Yearly, 40 a year".',
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
    { name: 'WithActions', title: 'A card with actions' },
    { name: 'Choice', title: 'Choosing one card of several' },
  ],
  gaps: [
    {
      missing: 'Choice cards reported as a radio group with a position.',
      needs: 'radio controls that take children, as checkbox does',
      today: 'Each card is a button reported as selected.',
    },
  ],
  related: ['Item', 'Button'],
};
