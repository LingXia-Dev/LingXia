import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Button',
  category: 'actions',
  status: 'preview',
  since: '2026-11-12',
  summary: 'Runs an action when pressed, with variants, sizes and a loading state',
  description:
    'Button starts an action: save, send, delete, open a dialog. Its `label` is both its text and its accessible name, ' +
    'and it can wait on an async action with `clickAction`. To open a menu use DropdownMenu.',
  keywords: ['button', 'action', 'submit', 'cta', 'primary', 'destructive', 'loading', 'press', 'tap'],
  propsType: 'ButtonProps',
  recipe: { map: 'buttonRecipe', parts: 'buttonParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The pressable box: fill, border, press and hover paint.' },
    { part: 'glyph', required: false, description: '`startIcon`, replaced by the spinner while loading, and `endIcon`; from the icon set in use, hidden from assistive tech.' },
    { part: 'icon', required: false, description: 'The trailing slot that holds `endContent`.' },
    { part: 'spinner', required: false, description: 'Spins while `isLoading` or a `clickAction` is pending.' },
    { part: 'label', required: true, description: 'The text; never truncated, so keep it short.' },
  ],
  accessibility: {
    pattern: 'button',
    name: 'label',
    description: ['disabledMessage, while disabled'],
    states: [
      { state: 'disabled', from: 'isDisabled' },
      { state: 'busy', from: 'isLoading, or a pending clickAction' },
    ],
    keyboard: [],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'primary label', pair: ['on-accent', 'accent'], minimum: '4.5:1' },
      { what: 'destructive label', pair: ['on-danger', 'danger'], minimum: '4.5:1' },
      { what: 'secondary label', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'secondary boundary', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'link label', pair: ['accent', 'canvas'], minimum: '4.5:1' },
      { what: 'disabled label on its 6% ink fill, on the page', pair: ['text-muted', 'text/6'], over: 'canvas', minimum: '4.5:1' },
      { what: 'disabled label on its 6% ink fill, on a card', pair: ['text-muted', 'text/6'], over: 'surface', minimum: '4.5:1' },
    ],
    notes: [
      'A loading button stays enabled and focusable, reports busy, and ignores presses.',
      'Disabled, every variant turns to a faint ink fill with muted text, so the label stays readable in both themes.',
      'Sizes sm (32) and md (40) are below the 44 pt touch minimum; on touch layouts prefer lg until hit slop lets the press area extend past the paint.',
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
    { name: 'WithIcons', title: 'With icons' },
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
      missing: 'The disabled reason shown to sighted users.',
      needs: 'a focusable disabled state, so a hint note can open from it',
      today: 'The reason is read as the description; nothing is drawn.',
    },
    {
      missing: 'Announcing that loading started.',
      needs: 'an announcement without a mounted live region',
      today: 'Busy state only; a page that needs speech keeps its own status line.',
    },
    {
      missing: 'A keyboard focus ring that shows on a filled button.',
      needs: 'ring offset: a gap between the box and its ring',
      today: 'A ring on the fill blends into it, so Button draws no focus style of its own.',
    },
  ],
  related: ['IconButton', 'ButtonGroup', 'DropdownMenu', 'Dialog'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Starts an action',
    order: 1,
    page: {
      lead: 'Use a button to start an action: save, send, delete or open a dialog. Keep one primary button per view.',
      hero: 'Hero',
      variants: [
        { name: 'VariantsEveryday', title: 'Primary for the main action, secondary and ghost for the rest' },
        { name: 'VariantDestructive', title: 'Destructive, for loss' },
        { name: 'VariantLink', title: 'Link, a quiet way out beside a form' },
        { name: 'WithIcons', title: 'An icon before the label for the kind of action, after it for where it leads' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small, medium and large; prefer large on touch layouts' }],
      states: [
        { name: 'StateDefault', title: 'Default' },
        { name: 'StateBusy', title: 'Busy' },
        { name: 'StateDisabled', title: 'Disabled' },
        { name: 'StateSecondaryDisabled', title: 'Disabled, secondary' },
      ],
      inContext: [
        { name: 'DisabledWithReason', title: 'A form footer that waits for consent' },
        { name: 'AsyncAction', title: 'Waiting on a Logic action' },
      ],
    },
  },
};
