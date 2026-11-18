import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'NavList',
  category: 'navigation',
  status: 'preview',
  since: '2026-11-14',
  summary: 'A sidebar\'s list of places, in sections that open and close, with the current one marked',
  description:
    'NavList is the column of places beside a workspace: mailboxes, projects, the pages of a document. ' +
    'Rows sit in sections; a section with a title opens and closes from its heading and remembers it. ' +
    'The current row sits on an accent wash and is read as selected.',
  keywords: ['nav list', 'navigation list', 'sidebar', 'side bar', 'source list', 'outline', 'menu', 'table of contents', 'tree'],
  propsType: 'NavListProps',
  recipe: { map: 'navListRecipe', parts: 'navListParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The tree: one Tab stop, named by `label`.' },
    { part: 'group', required: false, description: 'A section heading: a treeitem that opens and closes its rows.' },
    { part: 'groupLabel', required: false, description: 'The section title, small and muted.' },
    { part: 'groupChevron', required: false, description: 'Points down while open, along the line while closed; mirrors for right-to-left text.' },
    { part: 'item', required: true, description: 'One row per place: a treeitem, on the accent wash when current.' },
    { part: 'label', required: true, description: 'The row\'s text; one line, cut with an ellipsis.' },
    { part: 'end', required: false, description: '`endText`: a count or short value, read as part of the row\'s name.' },
    { part: 'current', required: false, description: 'An unseen mark, one per current row: its layout is when the row is brought into view.' },
  ],
  accessibility: {
    pattern: 'tree',
    name: 'label',
    description: [],
    states: [
      { state: 'selected', from: 'value' },
      { state: 'expanded', from: 'sections, open as `defaultOpenSections` lists them, then by a press on the heading' },
      { state: 'disabled', from: 'items[].isDisabled' },
    ],
    keyboard: [
      { keys: 'Tab', effect: 'Into the list, on the current row; again, out of it.' },
      { keys: 'Up, Down', effect: 'The previous or next row or heading.' },
      { keys: 'Home, End', effect: 'The first or last row.' },
      { keys: 'Enter, Space', effect: 'Opens the row, or opens or closes a section.' },
    ],
    contrast: [
      { what: 'row text', pair: ['text', 'canvas'], minimum: '4.5:1' },
      { what: 'section titles and counts', pair: ['text-muted', 'canvas'], minimum: '4.5:1' },
      { what: 'current row text', pair: ['text', 'accent-soft'], minimum: '4.5:1' },
      { what: 'current row count', pair: ['text-muted', 'accent-soft'], minimum: '4.5:1' },
    ],
    notes: [
      'The current row is read as selected, the others as not selected; a page that is current is not yet announced as "current page".',
      'Rows are 36 px tall; on touch screens the View grows each press target to the platform minimum.',
      'A row with `endText` is named "Inbox, 12", so the count is read once.',
      'In a sidebar that scrolls, the current row is brought into view when the list first appears and whenever another row becomes current from outside the list; a row in a closed section stays where it is.',
    ],
  },
  guidance: {
    do: [
      'Keep it beside the content it switches, and show the chosen place in that content at once.',
      'Use sections with titles for long lists, and open only the ones in use with `defaultOpenSections`.',
      'Keep labels to a word or two; the column is narrow.',
    ],
    dont: [
      'Use it for actions; it chooses where to look, a Button or a menu does things.',
      'Use it for a few peers on one page; that is Tabs.',
      'Nest sections inside sections; a NavList is one level of headings.',
    ],
  },
  examples: [
    { name: 'Sidebar', title: 'A sidebar' },
    { name: 'Closed', title: 'A section that starts closed' },
  ],
  gaps: [
    { missing: 'The current row read as "current page"', needs: '`aria-current` in the View', today: 'Read as selected' },
    { missing: 'Typing a name to jump to its row', needs: 'Type-ahead in a View `tree`, as `menu` has', today: 'Arrows, Home and End' },
  ],
  related: ['Tabs', 'Item'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A sidebar of places',
    order: 2,
    page: {
      lead: 'Use a nav list for the column of places beside a workspace.',
      hero: 'Sidebar',
      variants: [{ name: 'Closed', title: 'A section that starts closed' }],
    },
  },
};
