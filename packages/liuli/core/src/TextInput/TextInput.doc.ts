import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'TextInput',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-12',
  summary: 'Single-line text field with label, help text, status and an optional clear button',
  description:
    'TextInput is a labelled one-line editor for names, emails, codes, amounts and searches. ' +
    'Bind it to a `useLocal` cell for View-local drafts, or pass `value` and `onChange` when the app owns the text.',
  keywords: ['text input', 'text field', 'input', 'textbox', 'email', 'password', 'search', 'form'],
  propsType: 'TextInputProps',
  recipe: { map: 'textInputRecipe', parts: 'textInputParts' },
  anatomy: [
    { part: 'box', required: true, description: 'The bordered box; it takes the focus colour while the editor has focus and the status colour otherwise.' },
    { part: 'icon', required: false, description: '`startIcon` by name, or the magnifier when `type` is `search`, or `startContent` (a node such as a spinner); 16 px, muted, hidden from assistive tech.' },
    { part: 'input', required: true, description: 'The editor; it fills the box height so taps above and below the text land in it.' },
    { part: 'clear', required: false, description: 'Empties the text; shown while there is text and `hasClear` is on.' },
  ],
  accessibility: {
    pattern: 'textbox',
    name: 'label',
    description: ['description, or disabledMessage while disabled', 'the status message'],
    states: [
      { state: 'disabled', from: 'isDisabled' },
      { state: 'invalid', from: "status.type === 'error'" },
      { state: 'required', from: 'isRequired' },
    ],
    keyboard: [{ keys: 'the keyboard action key, Enter', effect: 'runs onSubmit' }],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'text', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'placeholder', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'box boundary', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'focus boundary', pair: ['focus', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'The clear button is a separate control named by `clearLabel`.',
      'Sizes sm (32) and md (40) are shorter than the 44 pt touch minimum; a finger still reaches them in a 44 pt band around the box, which the host adds without changing what is drawn.',
    ],
  },
  guidance: {
    do: [
      'Bind to useLocal for drafts; send the value to Logic on submit, not on every keystroke.',
      'Set type, inputMode, autoComplete and enterKeyHint so the right keyboard and autofill appear.',
      'Explain the fix in the status message.',
      'Check a value the person has finished typing in `onBlur`, not on every keystroke.',
      'To move focus to this field from code — after a failed submit, say — call `focus()` on the cell it is bound to: `const ok = await actions.signIn(); if (!ok) password.focus();`.',
      'To change what is typed as it is typed, such as upper-casing a code, control the field: `value={code.value} onChange={next => code.set(next.toUpperCase())}`.',
    ],
    dont: [
      'Leave out label because a placeholder is shown; use isLabelHidden when the label must not be drawn.',
      'Use type="number" for codes, phone numbers or card numbers; use inputMode="numeric".',
    ],
  },
  examples: [
    { name: 'Bound', title: 'Bound to local state, with validation' },
    { name: 'Search', title: 'Search with a clear button' },
    { name: 'Adornments', title: 'Fixed text before and after the value, and an optional field' },
    { name: 'Disabled', title: 'Disabled, with the reason' },
    { name: 'Checking', title: 'Checking what was typed' },
    { name: 'ReadOnly', title: 'Read-only' },
    { name: 'SignIn', title: 'A password, shown on request, and focus back on it after a failed sign-in' },
    { name: 'ShareLink', title: 'A button inside the box, after the value' },
  ],
  gaps: [
    {
      missing: 'Dirty tracking.',
      needs: 'a record of the value the field started with',
      today: 'Keep the starting value beside the draft and compare.',
    },
    {
      missing: 'Read-only text that can still be selected and copied.',
      needs: 'readOnly on editors',
      today: 'Show the value as selectable text instead.',
    },
    {
      missing: 'Moving focus to the next field from code when the app owns the value.',
      needs: 'a focus handle for controlled editors',
      today: 'Bound fields chain with enterKeyHint="next" or cell.focus().',
    },
  ],
  playground: { props: { value: '', label: 'Email', placeholder: 'you@example.com' } },
  related: ['Field'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'One line of text',
    order: 1,
    page: {
      lead: 'Use a text input for one line the person types: a name, an email, a code or a search.',
      hero: 'Bound',
      variants: [
        { name: 'Plain', title: 'Label and placeholder' },
        { name: 'Search', title: 'Search, with a clear button' },
        { name: 'Adornments', title: 'A currency sign, a web address start and a unit; the last field is marked optional' },
        { name: 'SignIn', title: 'Signing in: the password field shows its text on request' },
        { name: 'ShareLink', title: 'A copy button inside the box' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small, medium and large' }],
      states: [
        { name: 'StateEmpty', title: 'Empty' },
        { name: 'StateFilled', title: 'Filled' },
        { name: 'StateInvalid', title: 'Invalid' },
        { name: 'StateDisabled', title: 'Disabled' },
        { name: 'Checking', title: 'While checking' },
        { name: 'ReadOnly', title: 'Read-only' },
      ],
      inContext: [{ name: 'Disabled', title: 'A fixed value, with the reason' }],
    },
  },
};
