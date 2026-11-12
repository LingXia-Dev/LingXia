import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'CheckboxInput',
  category: 'form',
  status: 'lab',
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
    name: 'label then description',
    states: [
      { state: 'checked', from: 'value or bind' },
      { state: 'disabled', from: 'isDisabled' },
    ],
    keyboard: [],
    contrast: [
      { what: 'box boundary, unchecked', pair: ['line-strong', 'surface'], minimum: '3:1', proven: true },
      { what: 'box fill, checked', pair: ['accent', 'surface'], minimum: '3:1', proven: true },
      { what: 'check mark', pair: ['on-accent', 'accent'], minimum: '3:1', proven: true },
      { what: 'label', pair: ['text', 'surface'], minimum: '4.5:1', proven: true },
    ],
    notes: [
      'The name is the label followed by the help text, because a control cannot yet carry a separate description.',
      'The status message sits outside the control in a polite live region.',
    ],
  },
  guidance: {
    do: [
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
  ],
  gaps: [
    {
      missing: 'A mixed state for a select-all box.',
      needs: 'a mixed checked state on the checkbox primitive',
      today: 'Two states only.',
    },
    {
      missing: 'Help text announced as a description rather than inside the name; invalid reported.',
      needs: 'aria-describedby and aria-invalid',
      today: 'The name is label plus help text; the status is read from its live region.',
    },
  ],
  related: ['Field'],
};
