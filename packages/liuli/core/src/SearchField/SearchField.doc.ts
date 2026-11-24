import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'SearchField',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A field for searching what is on the page, with a clear button and a line that says what was found',
  description:
    'SearchField is the field at the top of a list or a catalogue: a magnifier, the text, a button that clears it, and under it a line the app words to say what the search found. ' +
    'That line is announced when it changes, so a person who cannot see the list still learns the result. While a search runs elsewhere, a spinner takes the magnifier\'s place.',
  keywords: ['search', 'search field', 'search box', 'filter', 'find', 'query', 'magnifier'],
  propsType: 'SearchFieldProps',
  recipe: { map: 'searchFieldRecipe', parts: 'searchFieldParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The column: the field, then the results line. It is reported busy while loading.' },
    { part: 'spinner', required: false, description: 'Takes the magnifier\'s place while `isLoading`.' },
    { part: 'results', required: true, description: '`resultsLabel`: drawn under the field, or kept for assistive tech only; a status region, always mounted.' },
  ],
  accessibility: {
    pattern: 'textbox',
    name: 'label',
    description: [],
    states: [{ state: 'busy', from: 'isLoading' }, { state: 'disabled', from: 'isDisabled' }],
    keyboard: [
      { keys: 'Enter, or the keyboard\'s search key', effect: 'Runs `onSubmit` with the text' },
      { keys: 'Tab', effect: 'Moves from the text to the clear button while there is text' },
    ],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'text', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'placeholder, magnifier and results line', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'results line on the page', pair: ['text-muted', 'canvas'], minimum: '4.5:1' },
      { what: 'field edge', pair: ['line-strong', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'A search editor named by `label`; a placeholder is never the name, so `label` is required even when it is not drawn.',
      'The results line is a status region: each new `resultsLabel` is announced. Word it for the ear: "3 of 6 notes", "No notes match".',
      'The clear button is named by `clearLabel` and appears only while there is text.',
      'The on-screen keyboard\'s action key reads "search".',
    ],
  },
  guidance: {
    do: [
      'Filter as the person types when the list is on the device; use `onSubmit` when a search costs a request.',
      'Always pass `resultsLabel` once there is text, including when nothing matches.',
      'Hide the results line with `isResultsLabelHidden` only when the list under the field already shows the count.',
      'Keep the field where it is while results change, so typing is never interrupted.',
      'To move focus to this field from code — after a failed submit, say — call `focus()` on the cell it is bound to: `query.focus()`.',
      'At large text, give the field a line of its own rather than squeezing it beside a button, where it shrinks to its magnifier.',
      'In a column the field fills the width; in a row, a bar or toolbar, it is about twenty characters wide, as a native search input, and gives way when the row is short. Set a width with `className` only to change that.',
    ],
    dont: [
      'Use it to choose one value for a form; that is a Combobox.',
      'Clear the text when a search finds nothing; let the person edit it.',
      'Show a spinner for filtering that happens at once.',
    ],
  },
  examples: [
    { name: 'Notes', title: 'A list that narrows as you type' },
    { name: 'Loading', title: 'While a search runs' },
    { name: 'Labelled', title: 'With its label shown' },
    { name: 'Sizes', title: 'Sizes' },
  ],
  gaps: [
    {
      missing: 'Recent searches and suggestions under the field while typing.',
      needs: 'a popover anchored to an editor and opened from state, with key events from the editor',
      today: 'The page shows its own results under the field.',
    },
    {
      missing: 'A cancel button beside the field that ends the search, as a phone\'s search bar has.',
      needs: 'focus and blur events on an editor',
      today: 'The clear button empties the text; the page decides what an empty search shows.',
    },
  ],
  playground: { props: { value: '', label: 'Search', clearLabel: 'Clear search', placeholder: 'Search files' } },
  related: ['TextInput', 'Combobox'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Find in a list',
    order: 7,
    page: {
      lead: 'Use a search field above a list or a catalogue, so people can narrow it by typing.',
      hero: 'Notes',
      variants: [
        { name: 'Loading', title: 'A spinner in the magnifier\'s place while a search runs elsewhere' },
        { name: 'Labelled', title: 'With its label shown, among other fields' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small, medium and large' }],
      states: [
        { name: 'StateEmpty', title: 'Empty' },
        { name: 'StateFilled', title: 'With text and a result' },
        { name: 'StateNothing', title: 'Nothing found' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
      statesLayout: 'stack',
    },
  },
};
