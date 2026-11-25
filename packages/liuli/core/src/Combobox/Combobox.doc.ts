import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Combobox',
  category: 'selection',
  status: 'preview',
  since: '2026-11-15',
  summary: 'One choice from a long list, found by typing: in the field itself with a keyboard, in a sheet with a finger',
  description:
    'Combobox is a Select for lists too long to scan: countries, people, products. With a mouse and keyboard the field is the editor: typing narrows a list that hangs under it, ' +
    'the arrow keys move through it and Return chooses. With a finger the field is a button that opens a sheet from the bottom edge, with a search field over the list. ' +
    'Matching ignores case and looks in the label only: options whose label starts with what was typed come first, then those where a later word starts with it, ' +
    'then those that hold it inside a word, each group in the order the options were given. ' +
    'With `onQueryChange` the app replaces that: Combobox reports each change to the search text and shows `items` as passed, so the app filters and orders them by its own rule or a server\'s. ' +
    'It takes Field\'s label, help text and status.',
  keywords: ['combobox', 'autocomplete', 'typeahead', 'searchable select', 'picker', 'filter', 'search select', 'lookup'],
  propsType: 'ComboboxProps',
  recipe: { map: 'comboboxRecipe', parts: 'comboboxParts' },
  anatomy: [
    { part: 'trigger', required: true, description: 'The field\'s box and chevron. On touch a button showing the chosen option\'s label or the placeholder; with a mouse the box around the editor.' },
    { part: 'editor', required: false, description: 'With a mouse and keyboard: the text in the box. It shows the chosen option\'s label, and what is typed narrows the list.' },
    { part: 'value', required: true, description: 'The chosen option\'s label; read after the name, as a field\'s value is.' },
    { part: 'popover', required: false, description: 'With a mouse or trackpad: the list under the field, as wide as the field; focus stays in the text.' },
    { part: 'sheet', required: false, description: 'On touch: a sheet on the bottom edge, titled by `label`, holding a search field and the list.' },
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
      { keys: 'Typing', effect: 'In the field: opens the list and narrows it' },
      { keys: 'Down and Up', effect: 'Open the list, and move through the options that can be chosen; focus stays in the text' },
      { keys: 'Home and End', effect: 'With the list open: the first and last option' },
      { keys: 'Return', effect: 'Chooses the active option, or the first match of what was typed, and closes' },
      { keys: 'Escape', effect: 'Closes the list and puts the chosen option\'s label back' },
      { keys: 'Tab', effect: 'Leaves the field; what was typed and not chosen is dropped, and an emptied field clears the choice' },
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
      'The button pattern applies on touch: the field is a button named by `label`; the chosen option, then the help text and status, are read as its description, and it reports expanded while the sheet is open.',
      'With a mouse and keyboard the field is a combobox named by `label` whose value is its text. It reports expanded, and points at the active option, whose name is announced as the arrow keys move, while focus stays in the text.',
      'The options are a listbox named by `label`, each an option that reports selected; `emptyLabel` is announced when nothing matches.',
      'On touch the sheet is modal and titled by `label`; a tap outside, a drag down or Back closes it, and its search field is named by `searchLabel`.',
      'With a mouse the list has no backdrop: a click on an option chooses it without taking focus from the text, and a click elsewhere leaves the field, which closes the list.',
      'A screen reader hears a combobox on an editor as a text field today; the role and the list it controls are recorded for when the platform bridges carry them.',
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
    { name: 'Clearable', title: 'Clearable: the clear button takes the choice back' },
    { name: 'Assignee', title: 'Options with a second line' },
    { name: 'OwnMatching', title: 'The app does the matching' },
    { name: 'Sizes', title: 'Sizes' },
    { name: 'Shipping', title: 'In a form' },
  ],
  gaps: [
    {
      missing: 'The typeahead on a host built before editor keys, focus events and anchored popovers.',
      needs: 'a newer host',
      today: 'Not supported with a mouse on such a host; touch is unaffected.',
    },
    {
      missing: 'Typing in the field itself on touch.',
      needs: 'nothing from the View',
      today: 'A sheet with its own search field, which leaves room for the keyboard.',
    },
    {
      missing: 'Several choices at once, and clearing the choice on touch.',
      needs: 'nothing from the View',
      today: 'One choice. With a keyboard, emptying the field clears it; on touch an app that needs "none" adds an option for it.',
    },
  ],
  playground: { props: { value: '', label: 'Country', placeholder: 'Choose a country', searchLabel: 'Search countries', emptyLabel: 'No country matches' }, fixture: true },
  related: ['Select', 'TextInput', 'DropdownMenu'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Search, then choose',
    order: 6,
    page: {
      lead: 'Use a combobox to choose one option from a list too long to scan, by typing part of its name.',
      hero: 'Country',
      variants: [{ name: 'Clearable', title: 'Clearable: the clear button takes the choice back' }, 
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
