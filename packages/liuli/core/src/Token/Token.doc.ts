import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Token',
  category: 'selection',
  status: 'preview',
  since: '2026-11-15',
  summary: 'One chosen thing as a pill',
  description:
    'Token stands for one thing someone chose or typed: a recipient, a tag, a filter. It can carry a small button that takes it away, and its label can be a button of its own that opens the thing or toggles a filter. ' +
    'TokenList lays tokens out in a row that wraps and names them as a group. MultiSelect uses both for its chosen options.',
  keywords: ['token', 'chip', 'tag', 'pill', 'filter', 'recipient', 'removable'],
  propsType: 'TokenProps',
  recipe: { map: 'tokenRecipe', parts: 'tokenParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The pill, on a tint of the text colour.' },
    { part: 'icon', required: false, description: 'An icon before the label; not read.' },
    { part: 'dot', required: false, description: 'With `hue` and no icon: a dot in the hue; not read.' },
    { part: 'label', required: true, description: 'The name, cut short with an ellipsis when the row is narrow.' },
    { part: 'remove', required: false, description: 'A round button at the end, named by `removeLabel`.' },
    { part: 'list', required: false, description: 'TokenList: a wrapping row, a group named by `label`.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [
      { state: 'pressed', from: 'isSelected, on a pressable token' },
      { state: 'disabled', from: 'isDisabled' },
    ],
    keyboard: [
      { keys: 'Tab', effect: 'To a pressable token, and to each remove button' },
      { keys: 'Enter or Space', effect: 'Presses the token, or removes it' },
    ],
    target: { sm: 24, md: 32 },
    contrast: [
      { what: 'label on the pill', pair: ['text', 'text/8'], over: 'surface', minimum: '4.5:1' },
      { what: 'label, selected', pair: ['on-accent-soft', 'accent-soft'], minimum: '4.5:1' },
      { what: 'remove icon', pair: ['text-muted', 'text/8'], over: 'surface', minimum: '3:1' },
      { what: 'hue 1, label on its fill', pair: ['text', 'hue-1-soft'], minimum: '4.5:1' },
      { what: 'hue 2, label on its fill', pair: ['text', 'hue-2-soft'], minimum: '4.5:1' },
      { what: 'hue 3, label on its fill', pair: ['text', 'hue-3-soft'], minimum: '4.5:1' },
      { what: 'hue 4, label on its fill', pair: ['text', 'hue-4-soft'], minimum: '4.5:1' },
      { what: 'hue 5, label on its fill', pair: ['text', 'hue-5-soft'], minimum: '4.5:1' },
      { what: 'hue 6, label on its fill', pair: ['text', 'hue-6-soft'], minimum: '4.5:1' },
      { what: 'hue 7, label on its fill', pair: ['text', 'hue-7-soft'], minimum: '4.5:1' },
      { what: 'hue 8, label on its fill', pair: ['text', 'hue-8-soft'], minimum: '4.5:1' },
      { what: 'hue 1, the dot on its fill', pair: ['hue-1', 'hue-1-soft'], minimum: '3:1' },
      { what: 'hue 2, the dot on its fill', pair: ['hue-2', 'hue-2-soft'], minimum: '3:1' },
      { what: 'hue 3, the dot on its fill', pair: ['hue-3', 'hue-3-soft'], minimum: '3:1' },
      { what: 'hue 4, the dot on its fill', pair: ['hue-4', 'hue-4-soft'], minimum: '3:1' },
      { what: 'hue 5, the dot on its fill', pair: ['hue-5', 'hue-5-soft'], minimum: '3:1' },
      { what: 'hue 6, the dot on its fill', pair: ['hue-6', 'hue-6-soft'], minimum: '3:1' },
      { what: 'hue 7, the dot on its fill', pair: ['hue-7', 'hue-7-soft'], minimum: '3:1' },
      { what: 'hue 8, the dot on its fill', pair: ['hue-8', 'hue-8-soft'], minimum: '3:1' },
    ],
    notes: [
      'A plain token is read as text. A pressable one is a button; as a filter it reports pressed.',
      'The remove button is named with what it removes: "Remove Ana Lima".',
      'A token that opens and can be removed is two buttons side by side.',
      'The remove button is 24 or 32 px; a finger press that misses goes to the nearest one within reach.',
    ],
  },
  guidance: {
    do: [
      'Use tokens for things people chose or made: recipients, tags, applied filters.',
      'Say what is removed in `removeLabel`.',
    ],
    dont: [
      'Use a token for a status; that is a Badge.',
      'Use one for an action; that is a Button.',
    ],
  },
  examples: [
    { name: 'Recipients', title: 'People, each removable' },
    { name: 'Filters', title: 'Filters that toggle' },
    { name: 'Labels', title: 'Labels, each in its hue' },
    { name: 'Sizes', title: 'Sizes' },
  ],
  gaps: [],
  limits: [
    'Tokens are not reordered by dragging.',
    'Backspace in a field does not remove the last token; MultiSelect removes by each token\'s button.',
  ],
  related: ['MultiSelect', 'Badge', 'ToggleButton'],
  playground: { props: { label: 'Travel', size: 'md', isSelected: false, isDisabled: false } },
  gallery: {
    specimen: 'Specimen',
    tagline: 'A chosen thing',
    order: 8,
    page: {
      lead: 'Use a token for one thing someone chose: a recipient, a tag, a filter.',
      hero: 'Recipients',
      variants: [
        { name: 'Filters', title: 'Pressable, as filters that toggle' },
        { name: 'Labels', title: 'A hue for a category: the labels on an issue' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small (24) and medium (32), plain and removable' }],
      states: [{ name: 'StateDisabled', title: 'Disabled' }],
    },
  },
};
