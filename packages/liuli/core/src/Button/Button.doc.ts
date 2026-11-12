import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Button',
  category: 'action',
  status: 'lab',
  summary: 'Runs an action when pressed, with variants, sizes and a loading state',
  description:
    'Button starts an action: save, send, delete, open a dialog. Its `label` is both its text and its accessible name, ' +
    'and it can wait on an async action with `clickAction`. To open a menu use DropdownMenu.',
  keywords: ['button', 'action', 'submit', 'cta', 'primary', 'destructive', 'loading', 'press', 'tap'],
  propsType: 'ButtonProps',
  recipe: { map: 'buttonRecipe', parts: 'buttonParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The pressable box: fill, border, press and hover paint.' },
    { part: 'icon', required: false, description: '`startIcon`, replaced by the spinner while loading; a trailing slot holds `endContent`.' },
    { part: 'spinner', required: false, description: 'Spins while `isLoading` or a `clickAction` is pending.' },
    { part: 'label', required: true, description: 'The text; never truncated, so keep it short.' },
  ],
  accessibility: {
    pattern: 'button',
    name: 'label, then disabledMessage when disabled',
    states: [
      { state: 'disabled', from: 'isDisabled' },
      { state: 'busy', from: 'isLoading, or a pending clickAction' },
    ],
    keyboard: [],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'primary label', pair: ['on-accent', 'accent'], minimum: '4.5:1', proven: true },
      { what: 'destructive label', pair: ['on-danger', 'danger'], minimum: '4.5:1', proven: true },
      { what: 'secondary label', pair: ['text', 'surface'], minimum: '4.5:1', proven: true },
      { what: 'secondary boundary', pair: ['line-strong', 'surface'], minimum: '3:1', proven: true },
    ],
    notes: [
      'A loading button stays enabled and focusable, reports busy, and ignores presses.',
      'Sizes sm and md are under the 44 px touch target; prefer lg for a lone touch action until hit areas can extend past the paint.',
    ],
  },
  guidance: {
    do: [
      'Use one primary button per view, for the action most people take.',
      'Write the label as a verb phrase that names the outcome.',
      'Pass a Logic action to clickAction so the button shows progress by itself.',
      'Confirm destructive actions that cannot be undone with a Dialog.',
    ],
    dont: [
      'Put two primary buttons side by side.',
      'Build a button from a view and classes; only a button takes presses, focus and a role.',
      'Hard-code copy inside the button; labels come from the app.',
    ],
  },
  examples: [
    { name: 'Variants', title: 'Variants' },
    { name: 'Sizes', title: 'Sizes' },
    { name: 'AsyncAction', title: 'Waiting on a Logic action' },
    { name: 'DisabledWithReason', title: 'Disabled, with the reason' },
  ],
  gaps: [
    {
      missing: 'A 44 px press target around the sm and md sizes.',
      needs: 'hit slop: a press area larger than the painted box',
      today: 'The press area equals the box; the docs steer touch layouts to lg.',
    },
    {
      missing: 'The disabled reason read as a description, and shown to sighted users.',
      needs: 'aria-describedby, and a focusable disabled state with a hint popover',
      today: 'The reason joins the accessible name while disabled; nothing is drawn.',
    },
    {
      missing: 'Announcing that loading started.',
      needs: 'an announcement without a mounted live region',
      today: 'Busy state only; a page that needs speech keeps its own status line.',
    },
  ],
  related: ['DropdownMenu', 'Dialog'],
};
