import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'CheckboxList',
  category: 'selection',
  status: 'preview',
  since: '2026-11-15',
  summary: 'Several on/off choices under one question, any number of which can be chosen',
  description:
    'CheckboxList asks one question and offers a short list of answers that can each be on or off: which notifications, which days, which toppings. ' +
    'Its value is the list of chosen values, in the options\' order. The question names the group for assistive tech, and help text and a status message ' +
    'speak for the group as a whole, such as "Choose at least one".',
  keywords: ['checkbox list', 'checkbox group', 'multiple choice', 'multi select', 'options', 'checklist'],
  propsType: 'CheckboxListProps',
  recipe: { map: 'checkboxListRecipe', parts: 'checkboxListParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The group, named by `label`.' },
    { part: 'label', required: true, description: 'The question; hidden from assistive tech, since the group carries it as its name.' },
    { part: 'description', required: false, description: 'Help text under the question, read as the group\'s description.' },
    { part: 'options', required: true, description: 'The checkboxes, one CheckboxInput each.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: ['description', 'status message'],
    states: [{ state: 'checked', from: 'value, on each option' }, { state: 'disabled', from: 'isDisabled, or an option\'s own' }, { state: 'invalid', from: 'status.type === "error", on the group\'s message' }],
    keyboard: [
      { keys: 'Tab', effect: 'Moves from one checkbox to the next; each is its own stop' },
      { keys: 'Space', effect: 'Turns the focused checkbox on or off' },
    ],
    contrast: [
      { what: 'question and option labels', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'help text', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'A group named by `label`; each option is a checkbox named by its own label and described by its own help text.',
      'The group\'s status message is a live region under the options: it is announced when it appears or changes.',
      'Checkboxes are separate Tab stops, unlike radio buttons: each is its own answer.',
      'An option that cannot be chosen stays in the list, dimmed, with its reason as help text.',
    ],
  },
  guidance: {
    do: [
      'Word `label` as the question the options answer: "Notify me about".',
      'Keep it to about seven options; past that, group them or use a searchable list.',
      'Put a rule about the whole group ("Choose at least one") in `status`, shown once the choice breaks it.',
      'Keep an unavailable option visible with `isDisabled` and say why in its `description`.',
    ],
    dont: [
      'Use it when only one answer is allowed; that is a RadioGroup.',
      'Use it for a single on/off setting; that is a CheckboxInput or a Switch.',
      'Reorder options when they are chosen.',
    ],
  },
  examples: [
    { name: 'Notifications', title: 'Any number of options' },
    { name: 'AtLeastOne', title: 'A rule about the group' },
    { name: 'WithUnavailable', title: 'An option that cannot be chosen' },
    { name: 'Sizes', title: 'Sizes' },
  ],
  gaps: [
    {
      missing: 'A "select all" checkbox that shows a dash while some options are on.',
      needs: 'a mixed checked state on a checkbox',
      today: 'An app adds its own button to choose all or none.',
    },
    {
      missing: 'Options laid out in a row or in columns.',
      needs: 'nothing from the View',
      today: 'One column.',
    },
  ],
  related: ['CheckboxInput', 'RadioGroup', 'Switch'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Choose any number',
    order: 7,
    page: {
      lead: 'Use a checkbox list when a question has a few answers and any number of them can be chosen.',
      hero: 'Notifications',
      variants: [
        { name: 'AtLeastOne', title: 'A rule about the group, shown while the choice breaks it' },
        { name: 'WithUnavailable', title: 'An option that cannot be chosen stays in the list, with its reason' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small and large' }],
      states: [{ name: 'StateDisabled', title: 'Disabled' }],
    },
  },
};
