import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Combobox',
  category: 'selection',
  status: 'preview',
  since: '2026-11-15',
  summary: 'One choice from a long list, found by typing: a field that opens a search over its options',
  description:
    'Combobox is a Select for lists too long to scan: countries, people, products. The field shows the chosen option; pressing it opens a search field over the list, ' +
    'in a panel under the field for a mouse and in a sheet from the bottom edge for a finger. Typing narrows the list, and Return takes the first match. ' +
    'It takes Field\'s label, help text and status, and the app can do the matching itself, for its own rule or a server.',
  keywords: ['combobox', 'autocomplete', 'typeahead', 'searchable select', 'picker', 'filter', 'search select', 'lookup'],
  propsType: 'ComboboxProps',
  recipe: { map: 'comboboxRecipe', parts: 'comboboxParts' },
  anatomy: [
    { part: 'trigger', required: true, description: 'The field: the chosen option\'s label, or the placeholder, and a chevron. The button assistive tech names by `label`.' },
    { part: 'value', required: true, description: 'The chosen option\'s label; read after the name, as a field\'s value is.' },
    { part: 'popover', required: false, description: 'With a mouse or trackpad: a panel under the field holding the search field and the list.' },
    { part: 'sheet', required: false, description: 'On touch: a sheet on the bottom edge, titled by `label`, holding the same.' },
    { part: 'list', required: true, description: 'The matching options, scrolling under the search field, as a listbox; at most fifty are shown.' },
    { part: 'option', required: true, description: 'One option: its label, an optional second line, and a tick on the chosen one.' },
    { part: 'note', required: false, description: '`emptyLabel` when nothing matches, or `moreLabel` under a list cut at fifty.' },
  ],
  accessibility: {
    pattern: 'button',
    name: 'label',
    description: ['the chosen option', 'description', 'status message'],
    states: [
      { state: 'expanded', from: 'the open panel or sheet' },
      { state: 'selected', from: 'value, on the chosen option' },
      { state: 'disabled', from: 'isDisabled' },
      { state: 'modal', from: 'the sheet on touch' },
    ],
    keyboard: [
      { keys: 'Enter or Space', effect: 'On the field: opens the panel with focus in the search field' },
      { keys: 'Return', effect: 'In the search field: chooses the first match and closes' },
      { keys: 'Tab, then Up and Down', effect: 'From the search field into the list, and between options; Enter chooses' },
      { keys: 'Escape', effect: 'Closes without changing the choice; focus returns to the field' },
    ],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'chosen option in the field', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'placeholder', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'field edge', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'option in the panel', pair: ['text', 'surface-overlay'], minimum: '4.5:1' },
      { what: 'option\'s second line and the notes', pair: ['text-muted', 'surface-overlay'], minimum: '4.5:1' },
      { what: 'tick on the chosen option', pair: ['accent', 'surface-overlay'], minimum: '3:1' },
      { what: 'error edge', pair: ['danger', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'The field is a button named by `label`; the chosen option, then the help text and status, are read as its description. It reports expanded while open.',
      'The search field is named by `searchLabel`. The options are a listbox named by `label`, each an option that reports selected; `emptyLabel` is announced when nothing matches.',
      'On touch the sheet is modal and titled by `label`; a tap outside, a drag down or Back closes it. With a mouse a click outside or Escape closes the panel.',
      'Arrow keys do not move through the options while focus is in the search field: Return takes the first match, or Tab moves into the list.',
    ],
  },
  guidance: {
    do: [
      'Use it from about fifteen options up; below that a Select is quicker.',
      'Order options as people expect to find them: by name, or most used first.',
      'Write `emptyLabel` for what was searched: "No country matches".',
      'Use `onQueryChange` to match more than the label (a code, an email) or to fetch options; show `isLoading` while fetching.',
    ],
    dont: [
      'Use it for free text with suggestions; the field only ever holds one of its options.',
      'Put actions in the list; an option chooses, nothing else.',
      'Load thousands of options into it; fetch by what was typed.',
    ],
  },
  examples: [
    { name: 'Country', title: 'A long list, found by typing' },
    { name: 'Assignee', title: 'Options with a second line' },
    { name: 'OwnMatching', title: 'The app does the matching' },
    { name: 'Sizes', title: 'Sizes' },
    { name: 'Shipping', title: 'In a form' },
  ],
  gaps: [
    {
      missing: 'Typing in the field itself with suggestions under it, and arrow keys moving through them while typing.',
      needs: 'a popover anchored to an editor and opened from state, key events from an editor or an active option it points at, and the combobox role',
      today: 'The field is a button; the search field is inside the panel or sheet, and Return takes the first match.',
    },
    {
      missing: 'A panel as wide as its field.',
      needs: 'reading a laid-out box\'s size (`useBox`)',
      today: 'The panel is 288 points wide.',
    },
    {
      missing: 'Several choices at once, and clearing the choice.',
      needs: 'nothing from the View',
      today: 'One choice; an app that needs "none" adds an option for it.',
    },
  ],
  related: ['Select', 'TextInput', 'DropdownMenu'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Search, then choose',
    order: 6,
    page: {
      lead: 'Use a combobox to choose one option from a list too long to scan, by typing part of its name.',
      hero: 'Country',
      variants: [
        { name: 'Assignee', title: 'A second line for options with similar names; one that cannot be chosen is dimmed' },
        { name: 'OwnMatching', title: 'The app matches by its own rule: a code finds its airport' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small, medium and large' }],
      states: [
        { name: 'StateEmpty', title: 'Nothing chosen' },
        { name: 'StateChosen', title: 'Chosen' },
        { name: 'StateInvalid', title: 'Invalid' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
      statesLayout: 'stack',
      inContext: [{ name: 'Shipping', title: 'In a form' }],
    },
  },
};
