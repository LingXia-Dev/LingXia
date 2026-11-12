import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Select',
  category: 'inputs',
  status: 'preview',
  summary: 'One choice from a list that opens on demand',
  description:
    'Select picks one option from a list too long to show in place: a country, a role, a sort order. ' +
    'It is a Field around the select primitive, which opens the platform\'s own list or menu.',
  keywords: ['select', 'dropdown', 'picker', 'combo box', 'choose', 'options', 'menu', 'single select'],
  propsType: 'SelectProps',
  recipe: { map: 'selectRecipe', parts: 'selectParts' },
  anatomy: [
    { part: 'field', required: true, description: 'Field: the label, help text and status.' },
    { part: 'control', required: true, description: 'The select primitive: the box, the chosen label or placeholder, the chevron, and the platform list.' },
  ],
  accessibility: {
    pattern: 'select',
    name: 'label',
    description: ['description, or disabledMessage while disabled', 'the status message'],
    states: [
      { state: 'disabled', from: 'isDisabled' },
      { state: 'invalid', from: "status.type === 'error'" },
      { state: 'required', from: 'isRequired' },
    ],
    keyboard: [
      { keys: 'Enter, Space', effect: 'Opens the list.' },
      { keys: 'Up, Down', effect: 'Moves through the open list; Enter chooses, Escape closes.' },
    ],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'box boundary', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'chosen label', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'placeholder', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'The placeholder is never the name: the label is, and it stays when the label is hidden.',
      'The open list is the platform\'s, so it reads and scrolls as the device\'s own pickers do.',
    ],
  },
  guidance: {
    do: [
      'Use a select for seven or more options, or when the options matter less than the space.',
      'Write the placeholder as an instruction ("Choose a country"), not as an option.',
    ],
    dont: [
      'Use a select for two to six options that fit; use a RadioGroup so they can be compared.',
      'Use a select to run an action; use a DropdownMenu.',
    ],
  },
  examples: [
    { name: 'Country', title: 'With a placeholder and help text' },
    { name: 'Required', title: 'Required, with an error and a disabled option' },
    { name: 'Sizes', title: 'Sizes' },
  ],
  gaps: [
    {
      missing: 'A placeholder colour set the usual way.',
      needs: 'the class compiler to accept `placeholder:` on `<select>`; the renderer already reads the select\'s placeholder colour',
      today: 'Liuli tints the whole box text muted while nothing is chosen.',
    },
    {
      missing: 'A quieter chevron than the chosen label.',
      needs: 'a chevron colour on the select primitive apart from its text colour',
      today: 'The chevron takes the text colour.',
    },
    {
      missing: 'A long chosen label cut short with an ellipsis.',
      needs: 'the select to paint its label on one line and truncate, as its layout already measures it',
      today: 'Liuli sets `whitespace-nowrap`, so the label stays on one line; one wider than the box is clipped, not ellipsized.',
    },
  ],
  related: ['RadioGroup', 'DropdownMenu', 'Field'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'One from a long list',
    order: 2,
    page: {
      lead: 'Use a select when there are too many options to show at once, or the choice matters less than the space.',
      hero: 'Country',
      sizes: [{ name: 'Sizes', title: 'Small, medium and large' }],
      states: [
        { name: 'StateEmpty', title: 'Nothing chosen' },
        { name: 'StateChosen', title: 'Chosen' },
        { name: 'StateInvalid', title: 'Invalid' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
      inContext: [{ name: 'Required', title: 'Inviting someone, after a send with no role' }],
    },
  },
};
