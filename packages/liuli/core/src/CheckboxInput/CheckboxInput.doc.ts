import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'CheckboxInput',
  category: 'selection',
  status: 'preview',
  since: '2026-11-12',
  summary: 'One on/off choice with a label, help text and status',
  description:
    'CheckboxInput is a single yes/no choice: opt-ins, terms, settings that take effect on save. ' +
    'The box, label and help text form one control, so a tap anywhere on them toggles it.',
  keywords: ['checkbox', 'check', 'tick', 'opt in', 'agree', 'terms', 'toggle', 'boolean'],
  propsType: 'CheckboxInputProps',
  recipe: { map: 'checkboxInputRecipe', parts: 'checkboxInputParts' },
  anatomy: [
    { part: 'control', required: true, description: 'The checkbox primitive; the press target and the accessible node.' },
    { part: 'box', required: true, description: 'The square Liuli draws from the value; the primitive draws none when it has content.' },
    { part: 'mark', required: false, description: 'The check, shown while checked.' },
    { part: 'label', required: true, description: 'The choice; visually hidden with `isLabelHidden`.' },
    { part: 'description', required: false, description: 'Help text, or `disabledMessage` while disabled.' },
  ],
  accessibility: {
    pattern: 'checkbox',
    name: 'label',
    description: ['description, or disabledMessage while disabled', 'the status message'],
    states: [
      { state: 'checked', from: 'value or bind' },
      { state: 'disabled', from: 'isDisabled' },
      { state: 'invalid', from: "status.type === 'error'" },
      { state: 'required', from: 'isRequired' },
    ],
    keyboard: [],
    contrast: [
      { what: 'box boundary, unchecked', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'box fill, checked', pair: ['accent-solid', 'surface'], minimum: '3:1' },
      { what: 'check mark', pair: ['on-accent-solid', 'accent-solid'], minimum: '3:1' },
      { what: 'label', pair: ['text', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'The help text sits inside the control so a tap on it toggles, but it is read as the description, not the name.',
      'The status message sits outside the control in a polite live region and is also read as the description.',
      'The box is 16 to 24 px, but the press target is the whole row, label and help text included.',
    ],
  },
  guidance: {
    do: [
      'At large text, expect the box to keep its size, centred on the label\'s first line as that line grows; the label wraps, the mark is never squeezed.',
      'Phrase the label so that checked means yes.',
      'Use the help text for the consequence, not for a second question.',
    ],
    dont: [
      'Act on the change at once when the screen has a Save button; apply on save.',
      'Put another control inside the label.',
    ],
  },
  examples: [
    { name: 'Basic', title: 'With help text' },
    { name: 'Controlled', title: 'Owned by the app, with an error' },
    { name: 'Sizes', title: 'Sizes' },
    { name: 'StateMixed', title: 'Partly chosen' },
    { name: 'Saved', title: 'Saved by an action, undone if it fails' },
  ],
  gaps: [
    {
      missing: 'A mixed state for a select-all box.',
      needs: 'a mixed checked state on the checkbox primitive',
      today: 'Two states only.',
    },
  ],
  playground: { props: { value: false, label: 'Email me a weekly summary', description: 'Every Monday morning.' } },
  related: ['Field'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'One on/off choice',
    order: 1,
    page: {
      lead: 'Use a checkbox for one on/off choice that takes effect when the form is sent.',
      hero: 'Basic',
      sizes: [{ name: 'Sizes', title: 'Small, medium and large' }],
      states: [
        { name: 'StateOff', title: 'Off' },
        { name: 'StateOn', title: 'On' },
        { name: 'StateInvalid', title: 'Invalid' },
        { name: 'StateDisabled', title: 'Disabled' },
        { name: 'StateMixed', title: 'Mixed' },
      ],
      inContext: [
        { name: 'Controlled', title: 'Terms on a sign-up form, with an error' },
        { name: 'Saved', title: 'Saved by an action: busy while it runs, undone with a note if it fails' },
      ],
    },
  },
};
